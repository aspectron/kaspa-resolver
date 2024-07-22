use crate::imports::*;

#[allow(dead_code)]
pub const BIAS_SCALE: u64 = 1_000_000;

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:016x}:{:016x}] [{:>4}] {}",
            self.system_id(),
            self.node.uid(),
            self.sockets(),
            self.node.address
        )
    }
}

#[derive(Debug)]
pub struct Connection {
    caps: Arc<OnceLock<Caps>>,
    is_synced: AtomicBool,
    sockets: AtomicU64,
    node: Arc<NodeConfig>,
    monitor: Arc<Monitor>,
    params: PathParams,
    client: rpc::Client,
    shutdown_ctl: DuplexChannel<()>,
    delegate: ArcSwap<Option<Arc<Connection>>>,
    is_connected: AtomicBool,
    is_online: AtomicBool,
    args: Arc<Args>,
}

impl Connection {
    pub fn try_new(
        monitor: Arc<Monitor>,
        node: Arc<NodeConfig>,
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
            caps: Arc::new(OnceLock::new()),
            monitor,
            params,
            node,
            client,
            shutdown_ctl: DuplexChannel::oneshot(),
            delegate: ArcSwap::new(Arc::new(None)),
            is_connected: AtomicBool::new(false),
            is_synced: AtomicBool::new(false),
            sockets: AtomicU64::new(0),
            is_online: AtomicBool::new(false),
            args: args.clone(),
        })
    }

    #[inline]
    pub fn verbose(&self) -> bool {
        self.args.verbose
    }

    #[inline]
    pub fn score(&self) -> u64 {
        self.sockets.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn is_available(&self) -> bool {
        self.is_delegate()
            && self.online()
            && self
                .caps
                .get()
                .is_some_and(|caps| caps.socket_capacity > self.sockets())
    }

    #[inline]
    pub fn connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn online(&self) -> bool {
        self.is_online.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn is_synced(&self) -> bool {
        self.is_synced.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn sockets(&self) -> u64 {
        self.sockets.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn caps(&self) -> Arc<OnceLock<Caps>> {
        self.caps.clone()
    }

    #[inline]
    pub fn system_id(&self) -> u64 {
        self.caps()
            .get()
            .map(|caps| caps.system_id)
            .unwrap_or_default()
    }

    #[inline]
    pub fn address(&self) -> &str {
        self.node.address.as_str()
    }

    #[inline]
    pub fn node(&self) -> &Arc<NodeConfig> {
        &self.node
    }

    #[inline]
    pub fn network_id(&self) -> NetworkId {
        self.node.network
    }

    #[inline]
    pub fn is_delegate(&self) -> bool {
        self.delegate.load().is_none()
    }

    #[inline]
    pub fn delegate(self: &Arc<Self>) -> Arc<Connection> {
        match (**self.delegate.load()).clone() {
            Some(delegate) => delegate.delegate(),
            None => self.clone(),
        }
    }

    #[inline]
    pub fn bind_delegate(&self, delegate: Option<Arc<Connection>>) {
        self.delegate.store(Arc::new(delegate));
    }

    pub fn resolve_delegates(self: &Arc<Self>) -> Vec<Arc<Connection>> {
        let mut delegates = Vec::new();
        let mut delegate = (*self).clone();
        while let Some(next) = (**delegate.delegate.load()).clone() {
            delegates.push(next.clone());
            delegate = next;
        }
        delegates
    }

    pub fn status(&self) -> &'static str {
        if self.connected() {
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

        // TODO - delegate state changes inside `update_state()`!
        let mut interval = if self.is_delegate() {
            workflow_core::task::interval(SyncSettings::poll())
        } else {
            workflow_core::task::interval(SyncSettings::ping())
        };

        loop {
            select! {
                _ = interval.next().fuse() => {
                    if self.is_connected.load(Ordering::Relaxed) {
                        let previous = self.is_online.load(Ordering::Relaxed);
                        let online = self.update_state().await.is_ok();
                        self.is_online.store(online, Ordering::Relaxed);
                        if online != previous {
                            if self.verbose() {
                                log_error!("Offline","{}", self.node.address);
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
                                    log_success!("Connected","{}",self.node.address);
                                    self.is_connected.store(true, Ordering::Relaxed);
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

        if self.caps().get().is_none() {
            let caps = self.client.get_caps().await?;
            let delegate_key = Delegate::new(caps.system_id(), self.network_id());
            if let Err(err) = self.caps().set(caps) {
                log_error!("CAPS", "Error setting caps: {:?}", err);
            }
            let mut delegates = self.monitor.delegates().write().unwrap();

            if let Some(delegate) = delegates.get(&delegate_key) {
                self.bind_delegate(Some(delegate.clone()));
            } else {
                delegates.insert(delegate_key, self.clone());
                self.bind_delegate(None);
            }
        }

        match self.client.get_sync().await {
            Ok(is_synced) => {
                let previous_sync = self.is_synced.load(Ordering::Relaxed);
                self.is_synced.store(is_synced, Ordering::Relaxed);

                if is_synced {
                    match self.client.get_active_connections().await {
                        Ok(connections) => {
                            if self.verbose() {
                                let previous = self.sockets.load(Ordering::Relaxed);
                                if connections != previous {
                                    self.sockets.store(connections, Ordering::Relaxed);
                                    log_success!("Clients", "{self}");
                                }
                            } else {
                                self.sockets.store(connections, Ordering::Relaxed);
                            }

                            Ok(())
                        }
                        Err(err) => {
                            log_error!("Metrics", "{self}");
                            log_error!("RPC", "{err}");
                            Err(Error::Metrics)
                        }
                    }
                } else {
                    if is_synced != previous_sync {
                        log_error!("Syncing", "{self}");
                    }
                    Err(Error::Sync)
                }
            }
            Err(err) => {
                log_error!("RPC", "{self}");
                log_error!("RPC", "{err}");
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
    pub protocol: ProtocolKind,
    pub encoding: EncodingKind,
    pub encryption: TlsKind,
    pub network: &'a NetworkId,
    pub cores: u64,
    pub status: &'static str,
    pub connections: u64,
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
        let protocol = node.params().protocol();
        let encoding = node.params().encoding();
        let encryption = node.params().tls();
        let network = &node.network;
        let status = connection.status();
        let connections = delegate.sockets();
        let (version, sid, capacity, cores) = delegate
            .caps()
            .get()
            .map(|caps| {
                (
                    caps.version.clone(),
                    caps.system_id,
                    caps.socket_capacity,
                    caps.cpu_physical_cores,
                )
            })
            .unwrap_or_else(||("n/a".to_string(), 0, 0, 0));

        let delegates = connection
            .resolve_delegates()
            .iter()
            .map(|connection| format!("[{:016x}] {}", connection.system_id(), connection.address()))
            .collect::<Vec<String>>();
        let delegates = (!delegates.is_empty()).then_some(delegates);

        Self {
            sid,
            uid,
            version,
            url,
            protocol,
            encoding,
            encryption,
            network,
            cores,
            status,
            connections,
            capacity,
            delegates,
        }
    }
}
