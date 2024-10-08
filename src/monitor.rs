use crate::imports::*;

/// Monitor receives updates from [Connection] monitoring tasks
/// and updates the descriptors for each [Params] based on the
/// connection store (number of connections * bias).
pub struct Monitor {
    args: Arc<Args>,
    connections: RwLock<AHashMap<PathParams, Vec<Arc<Connection>>>>,
    delegates: RwLock<AHashMap<Delegate, Arc<Connection>>>,
    sorts: AHashMap<PathParams, AtomicBool>,
    channel: Channel<PathParams>,
    shutdown_ctl: DuplexChannel<()>,
    service: Service,
}

impl fmt::Debug for Monitor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Monitor")
            .field("verbose", &self.verbose())
            // .field("connections", &self.connections)
            .finish()
    }
}

impl Monitor {
    pub fn new(args: &Arc<Args>, service: Service) -> Self {
        let sorts = PathParams::iter_tls_any()
            .map(|params| (params, AtomicBool::new(false)))
            .collect();

        Self {
            args: args.clone(),
            connections: Default::default(),
            delegates: Default::default(),
            sorts,
            channel: Channel::unbounded(),
            shutdown_ctl: DuplexChannel::oneshot(),
            service,
        }
    }

    pub fn verbose(&self) -> bool {
        self.args.verbose
    }

    pub fn delegates(&self) -> &RwLock<AHashMap<Delegate, Arc<Connection>>> {
        &self.delegates
    }

    pub fn connections(&self) -> AHashMap<PathParams, Vec<Arc<Connection>>> {
        self.connections.read().unwrap().clone()
    }

    pub fn to_vec(&self) -> Vec<Arc<Connection>> {
        PathParams::iter_tls_strict()
            .filter_map(|params| self.connections.read().unwrap().get(&params).cloned())
            .flatten()
            .collect()
    }

    /// Process an update to `Server.toml` removing or adding node connections accordingly.
    pub async fn update_nodes(
        self: &Arc<Self>,
        global_node_list: &mut Vec<Arc<Node>>,
    ) -> Result<()> {
        let mut nodes = Vec::new();
        global_node_list.retain(|node| {
            if node.service() == self.service {
                nodes.push(node.clone());
                false
            } else {
                true
            }
        });

        let mut connections = self.connections();

        let mut tls_any_created = Vec::new();
        let mut tls_any_removed = Vec::new();

        for params in PathParams::iter_tls_strict() {
            let nodes = nodes
                .iter()
                .filter(|node| node.params() == &params)
                .collect::<Vec<_>>();

            let list = connections.entry(params).or_default();

            let create: Vec<_> = nodes
                .iter()
                .filter(|node| !list.iter().any(|connection| connection.node() == **node))
                .collect();

            let remove: Vec<_> = list
                .iter()
                .filter(|connection| !nodes.iter().any(|node| connection.node() == *node))
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
                list.push(created.clone());
                tls_any_created.push(created);
            }

            for removed in remove {
                removed.stop().await?;
                list.retain(|c| c.node() != removed.node());
                tls_any_removed.push(removed);
            }
        }

        // remove connections from TlsAny list
        tls_any_removed.into_iter().for_each(|connection| {
            let params = connection.params().to_tls(TlsKind::Any);
            let list = connections.entry(params).or_default();
            list.retain(|connection| connection.node() != connection.node());
        });

        // create connections in TlsAny list
        tls_any_created.into_iter().for_each(|connection| {
            let params = connection.params().to_tls(TlsKind::Any);
            let list = connections.entry(params).or_default();
            list.push(connection);
        });

        // collect all strict Tls connections and group them by network_uid (fqdn+network+tls)
        let targets = AHashMap::group_from(
            connections
                .iter()
                .filter_map(|(params, list)| params.is_tls_strict().then_some(list))
                .flatten()
                .map(|connection| {
                    (
                        connection.node().network_node_uid(),
                        connection.node().transport_kind(),
                        connection.clone(),
                    )
                }),
        );

        for (_network_uid, transport_map) in targets.iter() {
            if let Some(wrpc_borsh) = transport_map.get(&TransportKind::WrpcBorsh) {
                if let Some(wrpc_json) = transport_map.get(&TransportKind::WrpcJson) {
                    wrpc_json.bind_delegate(Some(wrpc_borsh.clone()));
                } else if let Some(grpc) = transport_map.get(&TransportKind::Grpc) {
                    grpc.bind_delegate(Some(wrpc_borsh.clone()));
                }
            }
        }

        if self.args.debug {
            for params in PathParams::iter_tls_any() {
                println!("{}:{}", self.service, params);
                if let Some(connections) = connections.get(&params) {
                    if connections.is_empty() {
                        println!("\t- None (0)");
                    } else {
                        for connection in connections {
                            println!("\t- {}", connection);
                        }
                    }
                } else {
                    println!("\t- N/A");
                }
            }
        }

        *self.connections.write().unwrap() = connections;

        Ok(())
    }

    pub async fn start(self: &Arc<Self>) -> Result<()> {
        let this = self.clone();
        spawn(async move {
            if let Err(error) = this.task().await {
                println!("Monitor task error: {:?}", error);
            }
        });

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

        let mut interval = workflow_core::task::interval(Duration::from_millis(300));

        loop {
            select! {

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

    pub fn schedule_sort(&self, params_tls_kind: &PathParams) {
        self.sorts
            .get(params_tls_kind)
            .unwrap()
            .store(true, Ordering::Relaxed);

        let params_tls_any = params_tls_kind.to_tls(TlsKind::Any);
        self.sorts
            .get(&params_tls_any)
            .unwrap()
            .store(true, Ordering::Relaxed);
    }

    // /// Get JSON string representing node information (id, url, provider, link)
    pub fn election(&self, params: &PathParams) -> Option<String> {
        if self.verbose() {
            println!("election for: {}", params);
        }

        let connections = self.connections.read().unwrap();

        const DELEGATES_ONLY: bool = true;

        if self.verbose() {
            if let Some(connections) = connections.get(params) {
                connections
                    .iter()
                    .filter(|connection| {
                        if DELEGATES_ONLY {
                            connection.is_delegate()
                        } else {
                            true
                        }
                    })
                    .for_each(|connection| {
                        println!("\t- {}", connection);
                    });
            } else {
                println!("\t- N/A");
            }
        }

        let connections = connections
            .get(params)?
            .iter()
            .filter(|connection| {
                if DELEGATES_ONLY {
                    connection.is_delegate() && connection.is_available()
                } else {
                    connection.is_available()
                }
            })
            .collect::<Vec<_>>();

        if !connections.is_empty() {
            let node = select_with_weighted_rng(connections);
            serde_json::to_string(&Output::from(node)).ok()
        } else {
            None
        }
    }
}

fn select_with_weighted_rng(nodes: Vec<&Arc<Connection>>) -> &Arc<Connection> {
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
