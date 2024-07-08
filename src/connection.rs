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
            self.node.id_string,
            self.clients(),
            self.node.address
        )
    }
}

#[derive(Debug)]
pub struct Connection<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    pub node: Arc<Node>,
    monitor: Arc<Monitor<T>>,
    params: PathParams,
    client: T,
    shutdown_ctl: DuplexChannel<()>,
    caps: OnceLock<Caps>,
    is_connected: AtomicBool,
    is_synced: AtomicBool,
    is_online: AtomicBool,
    clients: AtomicU64,
    args: Arc<Args>,
}

impl<T> Connection<T>
where
    T: rpc::Client + Send + Sync + 'static,
{
    pub fn try_new(
        monitor: Arc<Monitor<T>>,
        node: Arc<Node>,
        _sender: Sender<PathParams>,
        args: &Arc<Args>,
    ) -> Result<Self> {
        let params = node.params();
        let client = T::try_new(node.encoding, &node.address)?;
        Ok(Self {
            monitor,
            params,
            node,
            client,
            shutdown_ctl: DuplexChannel::oneshot(),
            caps: OnceLock::new(),
            is_connected: AtomicBool::new(false),
            is_synced: AtomicBool::new(false),
            is_online: AtomicBool::new(false),
            clients: AtomicU64::new(0),
            args: args.clone(),
        })
    }

    pub fn verbose(&self) -> bool {
        self.args.verbose
    }

    pub fn score(&self) -> u64 {
        self.clients.load(Ordering::Relaxed) // * self.bias / BIAS_SCALE
    }

    pub fn is_available(&self) -> bool {
        self.online()
            && self
                .caps
                .get()
                .map(|caps| caps.socket_capacity > self.clients())
                .unwrap_or(false)
    }

    pub fn connected(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }

    pub fn online(&self) -> bool {
        self.is_online.load(Ordering::Relaxed)
    }

    pub fn is_synced(&self) -> bool {
        self.is_synced.load(Ordering::Relaxed)
    }

    pub fn clients(&self) -> u64 {
        self.clients.load(Ordering::Relaxed)
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
        let options = ConnectOptions {
            block_async_connect: false,
            strategy: ConnectStrategy::Retry,
            ..Default::default()
        };

        self.client.connect(options).await?;
        Ok(())
    }

    async fn task(self: Arc<Self>) -> Result<()> {
        self.connect().await?;
        let rpc_ctl_channel = self.client.multiplexer().channel();
        let shutdown_ctl_receiver = self.shutdown_ctl.request.receiver.clone();
        let shutdown_ctl_sender = self.shutdown_ctl.response.sender.clone();

        let interval = workflow_core::task::interval(Duration::from_secs(1));
        pin_mut!(interval);

        loop {
            select! {
                _ = interval.next().fuse() => {
                    if self.is_connected.load(Ordering::Relaxed) {
                        let previous = self.is_online.load(Ordering::Relaxed);
                        let online = self.update_metrics().await.is_ok();
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
                                    if self.update_metrics().await.is_ok() {
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

    async fn update_metrics(self: &Arc<Self>) -> Result<()> {
        if self.caps.get().is_none() {
            let caps = self.client.get_caps().await?;
            self.caps.set(caps).unwrap();
        }

        match self.client.get_sync().await {
            Ok(is_synced) => {
                let previous_sync = self.is_synced.load(Ordering::Relaxed);
                self.is_synced.store(is_synced, Ordering::Relaxed);

                if is_synced {
                    match self.client.get_active_connections().await {
                        Ok(connections) => {
                            if self.verbose() {
                                let previous = self.clients.load(Ordering::Relaxed);
                                if connections != previous {
                                    self.clients.store(connections, Ordering::Relaxed);
                                    log_success!("Clients", "{self}");
                                }
                            } else {
                                self.clients.store(connections, Ordering::Relaxed);
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
    pub id: &'a str,
    pub url: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_url: Option<&'a str>,
}

impl<'a, T> From<&'a Arc<Connection<T>>> for Output<'a>
where
    T: rpc::Client + Send + Sync + 'static,
{
    fn from(connection: &'a Arc<Connection<T>>) -> Self {
        let id = connection.node.id_string.as_str();
        let url = connection.node.address.as_str();
        let provider_name = connection
            .node
            .provider
            .as_ref()
            .map(|provider| provider.name.as_str());
        let provider_url = connection
            .node
            .provider
            .as_ref()
            .map(|provider| provider.url.as_str());

        // let provider_name = connection.node.provider.as_deref();
        // let provider_url = connection.node.link.as_deref();
        Self {
            id,
            url,
            provider_name,
            provider_url,
        }
    }
}
