use crate::imports::*;

#[allow(dead_code)]
pub const BIAS_SCALE: u64 = 1_000_000;

impl<T> fmt::Display for Connection<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:016x}:{:016x}] [{:>4}] {}",
            // self.context().system_id_as_hex_str(),
            self.context().system_id(),
            self.node.uid(),
            self.clients(),
            self.node.address
        )
    }
}

#[derive(Debug)]
pub struct Context {
    caps: Arc<OnceLock<Caps>>,
    is_synced: AtomicBool,
    clients: AtomicU64,
    fqdn: String,
}

impl Context {
    pub fn new<S>(fqdn: S) -> Self 
    where S : Display
    {
        Self {
            caps: Arc::new(OnceLock::new()),
            is_synced: AtomicBool::new(false),
            clients: AtomicU64::new(0),
            fqdn: fqdn.to_string(),
        }
    }

    pub fn system_id(&self) -> u64 {
        self.caps.get().map(|caps|caps.system_id).unwrap_or_default()
    }
    // pub fn system_id_as_hex_str(&self) -> String {
    //     self.caps.get().map(|caps|caps.system_id_hex_string.to_string()).unwrap_or("n/a".to_string())
    // }
}

#[derive(Debug)]
pub struct Connection<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    // node status context (common to multiple connections to the same node)
    context: ArcSwap<Context>,
    node: Arc<NodeConfig>,
    monitor: Arc<Monitor<T>>,
    params: PathParams,
    client: T,
    shutdown_ctl: DuplexChannel<()>,
    is_delegate: AtomicBool,
    delegate: Mutex<Option<Delegate>>,
    is_connected: AtomicBool,
    is_online: AtomicBool,
    is_aggregator: AtomicBool,
    args: Arc<Args>,
}

impl<T> Connection<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    pub fn try_new(
        monitor: Arc<Monitor<T>>,
        node: Arc<NodeConfig>,
        _sender: Sender<PathParams>,
        args: &Arc<Args>,
    ) -> Result<Self> {
        let params = *node.params();
        let encoding = params
            .encoding()
            .wrpc_encoding()
            .ok_or(Error::ConnectionProtocolEncoding)?;
        let client = T::try_new(encoding, &node.address)?;

        let context = Arc::new(Context::new(node.fqdn()));

        // let is_aggregator = params.encoding() == EncodingKind::Borsh;

        Ok(Self {
            context: ArcSwap::new(context),
            monitor,
            params,
            node,
            client,
            shutdown_ctl: DuplexChannel::oneshot(),
            is_delegate: AtomicBool::new(false),
            delegate: Mutex::new(None),
            is_connected: AtomicBool::new(false),
            is_online: AtomicBool::new(false),
            is_aggregator: AtomicBool::new(true),
            args: args.clone(),
        })
    }

    #[inline]
    pub fn verbose(&self) -> bool {
        self.args.verbose
    }

    #[inline]
    pub fn score(&self) -> u64 {
        self.context.load().clients.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn is_available(&self) -> bool {
        self.is_delegate()
            && self.online()
            && self
                .context
                .load()
                .caps
                .get()
                .is_some_and(|caps| caps.socket_capacity > self.clients())
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
        self.context.load().is_synced.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn clients(&self) -> u64 {
        self.context.load().clients.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn caps(&self) -> Arc<OnceLock<Caps>> {
        self.context.load().caps.clone()
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
    pub fn is_aggregator(&self) -> bool {
        // self.is_aggregator
        self.is_aggregator.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn bind_context(&self, context: Arc<Context>) {
        self.context.store(context);
        self.is_aggregator.store(false, Ordering::Relaxed);
    }

    #[inline]
    pub fn context(&self) -> Arc<Context> {
        self.context.load().clone()
    }

    #[inline]
    pub fn is_delegate(&self) -> bool {
        self.is_delegate.load(Ordering::Relaxed)
    }

    #[inline]
    // pub fn delegate_fqdn(&self) -> Option<String> {
    pub fn delegate(&self) -> Option<Delegate> {
        self.delegate.lock().unwrap().clone()
        // (!self.is_delegate()).then(|| self.context().fqdn.clone())
        // self.context().fqdn.clone()
    }

    pub fn status(&self) -> &'static str {
        if self.connected() {
            if self.is_synced() {
                "online"
            } else {
                "syncing"
            }
        } else {
            "offline"
        }
    }

    // pub fn address(&self) -> Option<String> {
    //     if let Some(caps) = self.caps().get() {
    //         let address = self.node.address_template.replace("$*", caps.system_id_hex.as_str());
    //         Some(format!(
    //             "{}: {}",
    //             self.node.address_template,
    //             caps.socket_capacity
    //         ))
    //     } else {
    //         None
    //     }
    // }

    async fn connect(&self) -> Result<()> {
        self.client.connect().await?;
        Ok(())
    }

    async fn task(self: Arc<Self>) -> Result<()> {
        self.connect().await?;
        let rpc_ctl_channel = self.client.multiplexer().channel();
        let shutdown_ctl_receiver = self.shutdown_ctl.request.receiver.clone();
        let shutdown_ctl_sender = self.shutdown_ctl.response.sender.clone();

        let mut interval = if self.is_aggregator() {
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

        // if !self.is_aggregator() {
        //     // println!(" --- bailing ... is aggregator...");
        //     self.client.ping().await?;

        //     return Ok(());
        // }

        // println!("Updating state");
        if self.caps().get().is_none() {
            // println!(" $$$ GETTING CAPS $$$");
            let caps = self.client.get_caps().await?;
            // println!("Got caps: {:?}", caps);
            // let caps = caps?;
            // let caps = self.client.get_caps().await?;
            // println!("Got caps: {:?}", caps);
            // let delegate = Delegate::new(caps.system_id(), self.node.network_node_uid());
            let delegate = Delegate::new(caps.system_id(), self.network_id());

            // ! OnceLock can be hit multiple times ???
            // self.caps().set(caps).ok();
            if let Err(err) = self.caps().set(caps) {
                log_error!("CAPS","Error setting caps: {:?}", err);
            }

            let mut delegates = self.monitor.delegates().write().unwrap();

            // if delegates.contains_key(&delegate) {
            if let Some(_delegate) = delegates.get(&delegate) {
                println!("--- NOT A DELEGATE {}", self.node().address());
                self.is_delegate.store(false, Ordering::Relaxed);
                self.delegate.lock().unwrap().replace(delegate);
            } else {
                println!("I am a delegate {}", self.node().address());
                delegates.insert(delegate, self.clone());
                self.is_delegate.store(true, Ordering::Relaxed);
                *self.delegate.lock().unwrap() = None;
            }

            // match delegates.entry(delegate) {
            //     Entry::Vacant(e) => {
            //         println!("------ DELEGATE ------");
            //         e.insert(self.clone());
            //         self.is_delegate.store(true, Ordering::Relaxed);
            //         self.delegate.lock().unwrap().take();
            //     }
            //     Entry::Occupied(e) => {
            //         self.is_delegate.store(false, Ordering::Relaxed);
            //         self.delegate.lock().unwrap().replace(delegate);
            //     }
            // // } else {
            //     // println!("!!!!!!! NOT A DELEGATE !!!!!");
            // };
            // if let Entry::Vacant(e) = delegates.entry(delegate) {
            //     println!("------ DELEGATE ------");
            //     e.insert(self.clone());
            //     self.is_delegate.store(true, Ordering::Relaxed);
            // } else {
            //     // println!("!!!!!!! NOT A DELEGATE !!!!!");
            //     self.is_delegate.store(false, Ordering::Relaxed);
            // };
        } 
        // else {
        //     println!("Caps already set");
        // }

        // if !self.is_aggregator() {
        //     // println!(" --- bailing ... is aggregator...");
        //     self.client.ping().await?;

        //     return Ok(());
        // }


        if !self.is_delegate() || !self.is_aggregator() {
            // println!("ping...");

            // self.client.ping().await?;
            if let Err(err) = self.client.ping().await {
                log_error!("Ping","{err}");
            }

            return Ok(());
        }

        // println!("## ==> GETTING SYNC STATUS");

        match self.client.get_sync().await {
            Ok(is_synced) => {
                let context = self.context();
                let previous_sync = context.is_synced.load(Ordering::Relaxed);
                context.is_synced.store(is_synced, Ordering::Relaxed);

                if is_synced {
                    match self.client.get_active_connections().await {
                        Ok(connections) => {
                            if self.verbose() {
                                let previous = context.clients.load(Ordering::Relaxed);
                                if connections != previous {
                                    context.clients.store(connections, Ordering::Relaxed);
                                    log_success!("Clients", "{self}");
                                }
                            } else {
                                context.clients.store(connections, Ordering::Relaxed);
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

impl<'a, T> From<&'a Arc<Connection<T>>> for Output<'a>
where
    T: rpc::Client + Send + Sync + 'static,
{
    fn from(connection: &'a Arc<Connection<T>>) -> Self {
        Self {
            uid: connection.node.uid_as_str(),
            url: connection.node.address(),
        }
    }
}


#[derive(Serialize)]
pub struct Status<'a> {
    #[serde(with = "SerHex::<Compact>")]
    pub sid: u64,
    #[serde(with = "SerHex::<Compact>")]
    pub uid: u64,
    pub url: &'a str,
    pub protocol: ProtocolKind,
    pub encoding: EncodingKind,
    // pub protocol: &'static str,
    // pub encoding: &'static str,
    pub tls: TlsKind,
    pub network: &'a NetworkId,
    pub cores: u64,
    // pub online: bool,
    pub status: &'static str,
    pub clients: u64,
    pub capacity: u64,
    pub aggregator : bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate: Option<Delegate>,
}

impl<'a, T> From<&'a Arc<Connection<T>>> for Status<'a>
where
    T: rpc::Client + Send + Sync + 'static,
{
    fn from(connection: &'a Arc<Connection<T>>) -> Self {
        let node = connection.node();
        let uid = node.uid();
        let url = node.address.as_str();
        let protocol = node.params().protocol();
        let encoding = node.params().encoding();
        let tls = node.params().tls();
        // let tls = node.tls;
        let network = &node.network;
        let status = connection.status();
        // let online = connection.online();
        let clients = connection.clients();
        let (sid, capacity, cores) = connection
            // .context()
            .caps()
            .get()
            .map(|caps| {
                (
                    caps.system_id,
                    caps.socket_capacity,
                    caps.cpu_physical_cores,
                )
            })
            .unwrap_or((0, 0, 0));
        let aggregator = connection.is_aggregator();
        let delegate = connection.delegate();

        println!("address : {}  delegate: {:?}", node.address(), delegate);

        Self {
            sid,
            uid,
            url,
            protocol,
            encoding,
            tls,
            network,
            cores,
            status,
            // online,
            clients,
            capacity,
            aggregator,
            delegate,
        }
    }
}
