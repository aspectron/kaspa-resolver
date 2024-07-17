use crate::imports::*;

/// Monitor receives updates from [Connection] monitoring tasks
/// and updates the descriptors for each [Params] based on the
/// connection store (number of connections * bias).
pub struct Monitor<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    args: Arc<Args>,
    connections: RwLock<AHashMap<PathParams, Vec<Arc<Connection<T>>>>>,
    delegates: RwLock<AHashMap<Vec<u8>, Arc<Connection<T>>>>,
    sorts: AHashMap<PathParams, AtomicBool>,
    channel: Channel<PathParams>,
    shutdown_ctl: DuplexChannel<()>,

    // ---
    _phantom: std::marker::PhantomData<T>,
}

impl<T> fmt::Debug for Monitor<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Monitor")
            .field("verbose", &self.verbose())
            // .field("connections", &self.connections)
            .finish()
    }
}

impl<T> Monitor<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            args: Arc::new(Args::default()),
            // name,
            connections: Default::default(),
            delegates: Default::default(),
            sorts: Default::default(),
            channel: Channel::unbounded(),
            shutdown_ctl: DuplexChannel::oneshot(),

            // ---
            _phantom: Default::default(),
        }
    }

    pub fn verbose(&self) -> bool {
        self.args.verbose
    }

    pub fn connections(&self) -> AHashMap<PathParams, Vec<Arc<Connection<T>>>> {
        self.connections.read().unwrap().clone()
    }

    /// Process an update to `Server.toml` removing or adding node connections accordingly.
    pub async fn update_nodes(self: &Arc<Self>, nodes: &[Arc<Node>]) -> Result<()> {
        let mut connections = self.connections();

        for params in PathParams::iter() {
            let nodes = nodes
                .iter()
                .filter(|node| node.params() == params)
                .collect::<Vec<_>>();

            let list = connections.entry(params).or_default();

            let create: Vec<_> = nodes
                .iter()
                .filter(|node| !list.iter().any(|connection| connection.node == ***node))
                .collect();

            let remove: Vec<_> = list
                .iter()
                .filter(|connection| !nodes.iter().any(|node| connection.node == **node))
                .cloned()
                .collect();

            for node in create {
                let created = Arc::new(Connection::try_new(
                    self.clone(),
                    (*node).clone(),
                    self.channel.sender.clone(),
                    &self.args,
                )?);
                created.start()?;
                list.push(created);
            }

            for removed in remove {
                removed.stop().await?;
                list.retain(|c| c.node != removed.node);
            }
        }

        // let targets = connections.values().flatten().cloned().collect::<Vec<_>>();
        let targets = AHashMap::group_from(
            connections
                .values()
                .flatten()
                .map(|c| (c.node.network_node_uid(), c.node.transport_kind, c.clone())),
        ); //.collect::<Vec<_>>();

        for (_network_uid, transport_map) in targets.iter() {
            if let Some(wrpc_borsh) = transport_map.get(&TransportKind::WrpcBorsh) {
                if let Some(wrpc_json) = transport_map.get(&TransportKind::WrpcJson) {
                    wrpc_json.set_sibling(Some(wrpc_borsh.clone().into()));
                } else if let Some(grpc) = transport_map.get(&TransportKind::Grpc) {
                    grpc.set_sibling(Some(wrpc_borsh.clone().into()));
                }
            }
        }

        *self.connections.write().unwrap() = connections;

        // flush all params to the update channel to refresh selected descriptors
        // PathParams::iter().for_each(|param| self.channel.sender.try_send(param).unwrap());

        Ok(())
    }

    pub async fn start(self: &Arc<Self>, nodes: &mut Vec<Arc<Node>>) -> Result<()> {
        let mut list = Vec::new();
        nodes.retain(|node| {
            if node.service() == T::service() {
                list.push(node.clone());
                false
            } else {
                true
            }
        });

        let this = self.clone();
        spawn(async move {
            if let Err(error) = this.task().await {
                println!("NodeConnection task error: {:?}", error);
            }
        });

        self.update_nodes(&list).await?;

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        self.shutdown_ctl
            .signal(())
            .await
            .expect("Monitor shutdown signal error");
        Ok(())
    }

    async fn task(self: Arc<Self>) -> Result<()> {
        let _receiver = self.channel.receiver.clone();
        let shutdown_ctl_receiver = self.shutdown_ctl.request.receiver.clone();
        let shutdown_ctl_sender = self.shutdown_ctl.response.sender.clone();

        let update = workflow_core::task::interval(Duration::from_secs(60 * 60 * 12));
        pin_mut!(update);

        let interval = workflow_core::task::interval(Duration::from_millis(300));
        pin_mut!(interval);

        loop {
            select! {

                _ = update.next().fuse() => {
                    if let Err(err) = crate::config::update().await {
                        log_error!("Update", "{}", err);
                    }
                }
                _ = interval.next().fuse() => {
                    for (params, sort) in self.sorts.iter() {
                        if sort.load(Ordering::Relaxed) {
                            sort.store(false, Ordering::Relaxed);

                            let mut connections = self.connections.write().unwrap();
                            if let Some(nodes) = connections.get_mut(params) {
                                nodes.sort_by_key(|connection| connection.score());
                            }
                        }
                    }
                }

                _ = shutdown_ctl_receiver.recv().fuse() => {
                    break;
                },

            }
        }

        shutdown_ctl_sender.send(()).await.unwrap();

        Ok(())
    }

    // /// Get the status of all nodes as a JSON string (available via `/status` endpoint if enabled).
    pub fn get_all(&self) -> Vec<Arc<Connection<T>>> {
        let connections = self.connections();
        let nodes = connections.values().flatten().cloned().collect::<Vec<_>>();
        nodes
    }

    pub fn schedule_sort(&self, params: &PathParams) {
        self.sorts
            .get(params)
            .unwrap()
            .store(true, Ordering::Relaxed);
    }

    // /// Get JSON string representing node information (id, url, provider, link)
    pub fn election(&self, params: &PathParams) -> Option<String> {
        let connections = self.connections.read().unwrap();
        let connections = connections
            .get(params)
            .expect("Monitor: expecting existing connection params")
            .iter()
            .filter(|connection| connection.is_available())
            .collect::<Vec<_>>();

        if !connections.is_empty() {
            let node = select_with_weighted_rng(connections);
            serde_json::to_string(&Status::from(node)).ok()
        } else {
            None
        }
    }
}

fn select_with_weighted_rng<T>(nodes: Vec<&Arc<Connection<T>>>) -> &Arc<Connection<T>>
where
    T: rpc::Client + Send + Sync + 'static,
{
    // Calculate total weight based on the position in the sorted list
    let total_weight: usize = nodes.iter().enumerate().map(|(i, _)| nodes.len() - i).sum();

    // Generate a random number within the range of total_weight
    let mut rng = rand::thread_rng();
    let mut rand_weight = rng.gen_range(0..total_weight);

    // Select a node based on the random weight
    for (i, node) in nodes.iter().enumerate() {
        let weight = nodes.len() - i;
        if rand_weight < weight {
            return node;
        }
        rand_weight -= weight;
    }

    // Fallback in case of error (shouldn't happen)
    nodes[0]
}

#[derive(Serialize)]
pub struct Status<'a> {
    pub id: &'a str,
    pub url: &'a str,
    pub protocol: &'static str,
    pub encoding: &'static str,
    pub tls: bool,
    pub network: &'a NetworkId,
    pub cores: u64,
    pub online: bool,
    pub status: &'static str,
    pub clients: u64,
    pub capacity: u64,
}

impl<'a, T> From<&'a Arc<Connection<T>>> for Status<'a>
where
    T: rpc::Client + Send + Sync + 'static,
{
    fn from(connection: &'a Arc<Connection<T>>) -> Self {
        let url = connection.node.address.as_str();
        let protocol = connection.node.transport_kind.protocol();
        let encoding = connection.node.transport_kind.encoding();
        let tls = connection.node.tls;
        let network = &connection.node.network;
        let status = connection.status();
        let online = connection.online();
        let clients = connection.clients();
        let (id, capacity, cores) = connection
            .caps()
            .map(|caps| {
                (
                    caps.hex_id.as_str(),
                    caps.socket_capacity,
                    caps.cpu_physical_cores,
                )
            })
            .unwrap_or(("n/a", 0, 0));
        Self {
            id,
            url,
            protocol,
            encoding,
            tls,
            network,
            cores,
            status,
            online,
            clients,
            capacity,
        }
    }
}
