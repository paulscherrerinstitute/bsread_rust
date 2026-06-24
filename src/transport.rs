#[derive(Clone, Debug)]
pub enum Transport {
    Tcp { port: u32, host: Option< &'static str> },
    Ipc { name: &'static str },
}

//pub fn get_local_address() -> String { "127.0.0.1".to_string()}
// pub fn get_local_address() -> &'static str {"*"}
pub fn get_local_address() -> &'static str {"0.0.0.0"}

pub fn get_ipc_feeds_folder() -> &'static str {"/tmp"}
impl Transport {
    pub fn endpoint(&self) -> String {
        match self {
            Transport::Tcp {port, host} => {
                let host = host
                    .clone()
                    .unwrap_or_else(get_local_address);

                if host.contains("://") {
                    format!("{}:{}", host, port)
                } else {
                    format!("tcp://{}:{}", host, port)
                }
            },
            Transport::Ipc { name } => {
                let folder = get_ipc_feeds_folder();
                //let path = Path::new(folder.as_str());
                //fs::create_dir_all(path).expect("Failed to create ipc feeds folder");
                format!("ipc://{}/bsread_icp_{}", folder, name)
            },
        }
    }
}