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
            "{}: [{:>3}] {}",
            self.node.uid_as_str(),
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
}

impl Context {
    pub fn new() -> Self {
        Self {
            caps: Arc::new(OnceLock::new()),
            is_synced: AtomicBool::new(false),
            clients: AtomicU64::new(0),
        }
    }
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
    is_connected: AtomicBool,
    is_online: AtomicBool,
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
        let params = node.params();
        let encoding = node
            .transport_kind
            .wrpc_encoding()
            .ok_or(Error::ConnectionProtocolEncoding)?;
        let client = T::try_new(encoding, &node.address)?;

        let context = Arc::new(Context::new());

        Ok(Self {
            context: ArcSwap::new(context),
            monitor,
            params,
            node,
            client,
            shutdown_ctl: DuplexChannel::oneshot(),
            is_delegate: AtomicBool::new(false),
            is_connected: AtomicBool::new(false),
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
    pub fn set_context(&self, context: Arc<Context>) {
        self.context.store(context);
    }

    #[inline]
    pub fn context(&self) -> Arc<Context> {
        self.context.load().clone()
    }

    #[inline]
    pub fn is_delegate(&self) -> bool {
        self.is_delegate.load(Ordering::Relaxed)
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

    async fn connect(&self) -> Result<()> {
        // let options = ConnectOptions {
        //     block_async_connect: false,
        //     strategy: ConnectStrategy::Retry,
        //     ..Default::default()
        // };

        self.client.connect().await?;
        Ok(())
    }

    async fn task(self: Arc<Self>) -> Result<()> {
        self.connect().await?;
        let rpc_ctl_channel = self.client.multiplexer().channel();
        let shutdown_ctl_receiver = self.shutdown_ctl.request.receiver.clone();
        let shutdown_ctl_sender = self.shutdown_ctl.response.sender.clone();

        let mut interval = workflow_core::task::interval(Duration::from_secs(1));
        // pin_mut!(interval);

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
        if self.caps().get().is_none() {
            let caps = self.client.get_caps().await?;

            let delegate = Delegate::new(caps.system_id(), self.node.network_node_uid());
            self.caps().set(caps).unwrap();

            let mut delegates = self.monitor.delegates();
            if let Entry::Vacant(e) = delegates.entry(delegate) {
                e.insert(self.clone());
                self.is_delegate.store(true, Ordering::Relaxed);
            } else {
                self.is_delegate.store(false, Ordering::Relaxed);
            };
        }

        if !self.is_delegate() {
            return Ok(());
        }

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
