use zmq::SocketType;
use crate::IOResult;
use crate::utils::app_name;

#[derive(Clone, Debug)]
pub enum Transport {
    Tcp { port: u32, host: Option<String> },
    Ipc { name: Option<String> },
}
pub const IPC_FILE_PREFIX:&str = "/bsread_icp_";

//pub fn local_address() -> String { "127.0.0.1".to_string()}
// pub fn local_address() -> &'static str {"*"}
pub fn local_address() -> &'static str {"0.0.0.0"}

pub fn ipc_feeds_folder() -> &'static str {"/tmp"}
impl Transport {
    pub fn endpoint(&self) -> String {
        match self {
            Transport::Tcp {port, host} => {
                let host = host.clone().unwrap_or(local_address().to_string());
                if host.contains("://") {
                    format!("{}:{}", host, port)
                } else {
                    format!("tcp://{}:{}", host, port)
                }
            },
            Transport::Ipc { name } => {
                let folder = ipc_feeds_folder();
                let suffix = match name{
                    None => {
                        match app_name(){
                            None => {"test".to_string()},
                            Some(app) => {app}
                        }
                    }
                    Some(str) => {str.clone()}
                };
                //let path = Path::new(folder.as_str());
                //fs::create_dir_all(path).expect("Failed to create ipc feeds folder");
                format!("ipc://{}{}{}", folder, IPC_FILE_PREFIX, suffix)
            },
        }
    }

    pub fn from_endpoint(endpoint: &str) -> Result<Self, String> {
        if let Some(rest) = endpoint.strip_prefix("tcp://") {
            let (host, port_str) = rest
                .rsplit_once(':')
                .ok_or_else(|| format!("Invalid TCP endpoint: {endpoint}"))?;

            let port = port_str
                .parse::<u32>()
                .map_err(|_| format!("Invalid TCP port: {port_str}"))?;

            let host = if host == local_address() {
                None
            } else {
                Some(host.to_string())
            };

            Ok(Transport::Tcp {port, host})
        } else if let Some(rest) = endpoint.strip_prefix("ipc://") {
            let (_, name) = rest
                .rsplit_once(IPC_FILE_PREFIX)
                .ok_or_else(|| format!("Invalid IPC endpoint: {endpoint}"))?;

            Ok(Transport::Ipc { name : Some(name.to_string())})
        } else {
            Err(format!("Unsupported endpoint: {endpoint}"))
        }
    }
}


pub trait SocketConfig {
    fn socket(&self) -> &zmq::Socket;
    fn transport(&self) -> Transport;
    fn socket_type(&self) -> SocketType;
    fn set_linger(&self, value: i32) -> IOResult<()> {
        self.socket().set_linger(value)?;
        Ok(())
    }

    fn set_rcvhwm(&mut self, value: i32)-> IOResult<()> {
        if let Err(e) =   self.socket().set_rcvhwm(value) {
            return Err(e.into());
        }
        Ok(())
    }

    fn set_sndhwm(&mut self, value: i32)-> IOResult<()> {
        if let Err(e) =   self.socket().set_sndhwm(value) {
            return Err(e.into());
        }
        Ok(())
    }

    fn set_keepalive(&self, idle: i32, intvl: i32, cnt: i32) -> IOResult<()> {
        match self.transport() {
            Transport::Tcp { .. } => {
                self.socket().set_tcp_keepalive(1)?;
                self.socket().set_tcp_keepalive_idle(idle)?;
                self.socket().set_tcp_keepalive_intvl(intvl)?;
                self.socket().set_tcp_keepalive_cnt(cnt)?;
            }
            Transport::Ipc { .. } => {
                log::info!(
                    "Ignoring keepalive on IPC endpoint {}",
                    self.transport().endpoint()
                );
            }
        }
        Ok(())
    }

    fn set_heartbeat(&self, ivl: i32, timeout: i32, ttl: i32) -> IOResult<()> {
        match self.transport() {
            Transport::Tcp { .. } => {
                self.socket().set_heartbeat_ivl(ivl)?;
                self.socket().set_heartbeat_timeout(timeout)?;
                self.socket().set_heartbeat_ttl(ttl)?;
            }
            Transport::Ipc { .. } => {
                log::info!(
                    "Ignoring heartbeat on IPC endpoint {}",
                    self.transport().endpoint()
                );
            }
        }
        Ok(())
    }
}