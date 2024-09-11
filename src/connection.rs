use crate::imports::*;

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let load = self
            .load()
            .map(|load| format!("{:1.2}%", load))
            .unwrap_or_else(|| "n/a  ".to_string());
        write!(
            f,
            "[{:016x}:{:016x}] [{:>4}] [{:>7}] {}",
            self.system_id(),
            self.node.uid(),
            self.clients(),
            load,
            self.node.address
        )
    }
}

#[derive(Debug)]
pub struct Connection {
    args: Arc<Args>,
    caps: ArcSwapOption<Caps>,
    is_synced: AtomicBool,
    clients: AtomicU64,
    peers: AtomicU64,
    node: Arc<Node>,
    monitor: Arc<Monitor>,
    params: PathParams,
    client: rpc::Client,
    shutdown_ctl: DuplexChannel<()>,
    delegate: ArcSwap<Option<Arc<Connection>>>,
    is_connected: AtomicBool,
    is_online: AtomicBool,
}

impl Connection {
    pub fn try_new(
        monitor: Arc<Monitor>,
        node: Arc<Node>,
        _sender: Sender<PathParams>,
        args: &Arc<Args>,
    ) -> Result<Self> {
        let params = *node.params();

        let client = match node.transport_kind {
            TransportKind::WrpcBorsh => {
                rpc::kaspa::Client::try_new(WrpcEncoding::Borsh, &node.address)?
            }
            TransportKind::WrpcJson => {
                rpc::kaspa::Client::try_new(WrpcEncoding::SerdeJson, &node.address)?
            }
            TransportKind::Grpc => {
                unimplemented!("gRPC support is not currently implemented")
            }
        };

        let client = rpc::Client::from(client);

        Ok(Self {
            args: args.clone(),
            caps: ArcSwapOption::new(None),
            monitor,
            params,
            node,
            client,
            shutdown_ctl: DuplexChannel::oneshot(),
            delegate: ArcSwap::new(Arc::new(None)),
            is_connected: AtomicBool::new(false),
            is_synced: AtomicBool::new(false),
            clients: AtomicU64::new(0),
            peers: AtomicU64::new(0),
            is_online: AtomicBool::new(false),
        })
    }

    #[inline]
    pub fn verbose(&self) -> bool {
        self.args.verbose
    }

    /// Represents the connection score, which is currently
    /// the number of sockets (clients + peers) the node has.
    #[inline]
    pub fn score(self: &Arc<Self>) -> u64 {
        self.delegate().sockets()
    }

    /// Connection availability state.
    #[inline]
    pub fn is_available(self: &Arc<Self>) -> bool {
        let delegate = self.delegate();

        self.is_connected()
            && delegate.is_online()
            && delegate.caps.load().as_ref().as_ref().is_some_and(|caps| {
                let clients = delegate.clients();
                let peers = delegate.peers();
                clients < caps.clients_limit && clients + peers < caps.fd_limit
            })
    }

    /// Indicates if the connection RPC is connected.
    #[inline]
    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    /// Indicates if the connection is available as a general
    /// concept: no errors have occurred during RPC calls
    /// and the node is in synced synced.
    #[inline]
    pub fn is_online(&self) -> bool {
        self.is_online.load(Ordering::Relaxed)
    }

    /// Indicates if the node is in synced state.
    #[inline]
    pub fn is_synced(&self) -> bool {
        self.is_synced.load(Ordering::Relaxed)
    }

    /// Number of RPC clients connected to the node.
    #[inline]
    pub fn clients(&self) -> u64 {
        self.clients.load(Ordering::Relaxed)
    }

    /// Number of p2p peers connected to the node.
    #[inline]
    pub fn peers(&self) -> u64 {
        self.peers.load(Ordering::Relaxed)
    }

    /// Total number of TCP sockets connected to the node.
    #[inline]
    pub fn sockets(&self) -> u64 {
        self.clients() + self.peers()
    }

    /// Connection load as a ratio of clients to capacity.
    pub fn load(&self) -> Option<f64> {
        self.caps
            .load()
            .as_ref()
            .map(|caps| self.clients() as f64 / caps.capacity as f64)
    }

    /// Node capabilities (partial system spec, see [`Caps`])
    #[inline]
    pub fn caps(&self) -> Option<Arc<Caps>> {
        self.caps.load().clone()
    }

    /// Unique system (machine) identifier of the node.
    #[inline]
    pub fn system_id(&self) -> u64 {
        self.caps
            .load()
            .as_ref()
            .map(|caps| caps.system_id)
            .unwrap_or_default()
    }

    /// Connection address (URL).
    #[inline]
    pub fn address(&self) -> &str {
        self.node.address.as_str()
    }

    /// Node configuration parameters used to create this connection.
    #[inline]
    pub fn node(&self) -> &Arc<Node> {
        &self.node
    }

    /// Connection parameters used to create this connection.
    #[inline]
    pub fn params(&self) -> PathParams {
        self.params
    }

    /// Network id of the node.
    #[inline]
    pub fn network_id(&self) -> NetworkId {
        self.node.network
    }

    /// Indicates if the connection is a delegate.
    #[inline]
    pub fn is_delegate(&self) -> bool {
        self.delegate.load().is_none()
    }

    /// Get the delegate of this connection.
    #[inline]
    pub fn delegate(self: &Arc<Self>) -> Arc<Connection> {
        match (**self.delegate.load()).clone() {
            Some(delegate) => delegate.delegate(),
            None => self.clone(),
        }
    }

    /// Associate a delegate to this connection. A delegate is a primary
    /// connection to the node that does actual performance monitoring
    /// while non-delegate connections remain idle in a keep-alive state.
    #[inline]
    pub fn bind_delegate(&self, delegate: Option<Arc<Connection>>) {
        self.delegate.store(Arc::new(delegate));
    }

    /// Creates a list of delegators for this connection, where the last
    /// entry is the delegate.
    pub fn resolve_delegators(self: &Arc<Self>) -> Vec<Arc<Connection>> {
        let mut delegates = Vec::new();
        let mut delegate = (*self).clone();
        while let Some(next) = (**delegate.delegate.load()).clone() {
            delegates.push(next.clone());
            delegate = next;
        }
        delegates
    }

    pub fn status(&self) -> &'static str {
        if self.is_connected() {
            if !self.is_delegate() {
                "delegator"
            } else if self.is_synced() {
                "online"
            } else {
                "syncing"
            }
        } else {
            "offline"
        }
    }

    async fn connect(&self) -> Result<()> {
        self.client.connect().await?;
        Ok(())
    }

    async fn task(self: Arc<Self>) -> Result<()> {
        self.connect().await?;
        let rpc_ctl_channel = self.client.multiplexer().channel();
        let shutdown_ctl_receiver = self.shutdown_ctl.request.receiver.clone();
        let shutdown_ctl_sender = self.shutdown_ctl.response.sender.clone();

        let mut ttl = TtlSettings::ttl();
        // TODO - delegate state changes inside `update_state()`!
        let mut poll = if self.is_delegate() {
            interval(SyncSettings::poll())
        } else {
            interval(SyncSettings::ping())
        };

        let mut last_connect_time: Option<Instant> = None;

        // use futures::StreamExt;
        loop {
            select! {

                _ = poll.next().fuse() => {

                    if TtlSettings::enable() {
                        if let Some(t) = last_connect_time {
                            if t.elapsed() > ttl {
                                // println!("-- t.elapsed(): {}", t.elapsed().as_millis());
                                last_connect_time = None;
                                // TODO reset caps ON ALL DELEGATES?
                                self.caps.store(None);
                                if self.is_connected.load(Ordering::Relaxed) {
                                    // log_info!("TTL","ttl disconnecting {}", self.node.address);
                                    self.client.disconnect().await.ok();
                                    // log_info!("TTL","Connecting {}", self.node.address);
                                    self.client.connect().await.ok();
                                }
                                continue;
                            }
                        }
                    }

                    if self.is_connected.load(Ordering::Relaxed) {
                        let previous = self.is_online.load(Ordering::Relaxed);
                        let online = self.update_state().await.is_ok();
                        self.is_online.store(online, Ordering::Relaxed);
                        if online != previous {
                            if self.verbose() {
                                if online {
                                    log_success!("Online","{}", self.node.address);
                                } else {
                                    log_error!("Offline","{}", self.node.address);
                                }
                            }
                            self.update();
                        }
                    }
                }

                msg = rpc_ctl_channel.receiver.recv().fuse() => {
                    match msg {
                        Ok(msg) => {

                            // handle wRPC channel connection and disconnection events
                            match msg {
                                Ctl::Connect => {
                                    last_connect_time = Some(Instant::now());
                                    ttl = TtlSettings::ttl();
                                    if self.args.verbose {
                                        log_info!("Connected","{} - ttl: {:1.2}",self.node.address,ttl.as_secs() as f64 / 60.0 / 60.0);
                                    } else {
                                        log_success!("Connected","{}",self.node.address);
                                    }
                                    self.is_connected.store(true, Ordering::Relaxed);
                                    // trigger caps reset
                                    self.caps.store(None);
                                    // update state
                                    if self.update_state().await.is_ok() {
                                        self.is_online.store(true, Ordering::Relaxed);
                                        self.update();
                                    } else {
                                        self.is_online.store(false, Ordering::Relaxed);
                                    }
                                },
                                Ctl::Disconnect => {
                                    self.is_connected.store(false, Ordering::Relaxed);
                                    self.is_online.store(false, Ordering::Relaxed);
                                    last_connect_time = None;
                                    self.update();
                                    log_error!("Disconnected","{}",self.node.address);
                                }
                            }
                        }
                        Err(err) => {
                            println!("Monitor: error while receiving rpc_ctl_channel message: {err}");
                            break;
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

    pub fn start(self: &Arc<Self>) -> Result<()> {
        let this = self.clone();
        spawn(async move {
            if let Err(error) = this.task().await {
                println!("NodeConnection task error: {:?}", error);
            }
        });

        Ok(())
    }

    pub async fn stop(self: &Arc<Self>) -> Result<()> {
        self.shutdown_ctl
            .signal(())
            .await
            .expect("NodeConnection shutdown signal error");
        Ok(())
    }

    async fn update_state(self: &Arc<Self>) -> Result<()> {
        if !self.is_delegate() {
            if let Err(err) = self.client.ping().await {
                log_error!("Ping", "{err}");
            }
            return Ok(());
        }

        if self.caps().is_none() {
            let last_system_id = self.caps().as_ref().map(|caps| caps.system_id());
            let caps = self.client.get_caps().await?;
            let system_id = caps.system_id();
            self.caps.store(Some(Arc::new(caps)));

            if last_system_id != Some(system_id) {
                let delegate_key = Delegate::new(system_id, self.network_id());
                let mut delegates = self.monitor.delegates().write().unwrap();
                if let Some(delegate) = delegates.get(&delegate_key) {
                    self.bind_delegate(Some(delegate.clone()));
                } else {
                    delegates.insert(delegate_key, self.clone());
                    self.bind_delegate(None);
                }
            }
        }

        match self.client.get_sync().await {
            Ok(is_synced) => {
                let previous_sync = self.is_synced.load(Ordering::Relaxed);
                self.is_synced.store(is_synced, Ordering::Relaxed);

                if is_synced {
                    match self.client.get_active_connections().await {
                        Ok(Connections { clients, peers }) => {
                            if self.verbose() {
                                let prev_clients = self.clients.load(Ordering::Relaxed);
                                let prev_peers = self.peers.load(Ordering::Relaxed);
                                if clients != prev_clients || peers != prev_peers {
                                    self.clients.store(clients, Ordering::Relaxed);
                                    self.peers.store(peers, Ordering::Relaxed);
                                    log_success!("Clients", "{self}");
                                }
                            } else {
                                self.clients.store(clients, Ordering::Relaxed);
                                self.peers.store(peers, Ordering::Relaxed);
                            }

                            Ok(())
                        }
                        Err(err) => {
                            log_error!("RPC", "{self}");
                            log_error!("Error", "{err}");
                            Err(Error::Metrics)
                        }
                    }
                } else {
                    if is_synced != previous_sync {
                        log_error!("Sync", "{self}");
                    }
                    Err(Error::Sync)
                }
            }
            Err(err) => {
                log_error!("RPC", "{self}");
                log_error!("Error", "{err}");
                Err(Error::Status)
            }
        }
    }

    #[inline]
    pub fn update(&self) {
        self.monitor.schedule_sort(&self.params);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Output<'a> {
    pub uid: &'a str,
    pub url: &'a str,
}

impl<'a> From<&'a Arc<Connection>> for Output<'a> {
    fn from(connection: &'a Arc<Connection>) -> Self {
        Self {
            uid: connection.node.uid_as_str(),
            url: connection.node.address(),
        }
    }
}

#[derive(Serialize)]
pub struct Status<'a> {
    pub version: String,
    #[serde(with = "SerHex::<Strict>")]
    pub sid: u64,
    #[serde(with = "SerHex::<Strict>")]
    pub uid: u64,
    pub url: &'a str,
    pub fqdn: &'a str,
    pub service: String,
    // pub service: &'a str,
    pub protocol: ProtocolKind,
    pub encoding: EncodingKind,
    pub encryption: TlsKind,
    pub network: &'a NetworkId,
    pub cores: u64,
    pub memory: u64,
    pub status: &'static str,
    pub peers: u64,
    pub clients: u64,
    pub capacity: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegates: Option<Vec<String>>,
}

impl<'a> From<&'a Arc<Connection>> for Status<'a> {
    fn from(connection: &'a Arc<Connection>) -> Self {
        let delegate = connection.delegate();

        let node = connection.node();
        let uid = node.uid();
        let url = node.address.as_str();
        let fqdn = node.fqdn.as_str();
        let service = node.service().to_string();
        let protocol = node.params().protocol();
        let encoding = node.params().encoding();
        let encryption = node.params().tls();
        let network = &node.network;
        let status = connection.status();
        let clients = delegate.clients();
        let peers = delegate.peers();
        let (version, sid, capacity, cores, memory) = delegate
            .caps()
            .as_ref()
            .as_ref()
            .map(|caps| {
                (
                    caps.version.clone(),
                    caps.system_id,
                    caps.clients_limit,
                    caps.cpu_physical_cores,
                    caps.total_memory,
                )
            })
            .unwrap_or_else(|| ("n/a".to_string(), 0, 0, 0, 0));

        let delegates = connection
            .resolve_delegators()
            .iter()
            .map(|connection| format!("[{:016x}] {}", connection.system_id(), connection.address()))
            .collect::<Vec<String>>();
        let delegates = (!delegates.is_empty()).then_some(delegates);

        Self {
            sid,
            uid,
            version,
            fqdn,
            service,
            url,
            protocol,
            encoding,
            encryption,
            network,
            cores,
            memory,
            status,
            clients,
            peers,
            capacity,
            delegates,
        }
    }
}
