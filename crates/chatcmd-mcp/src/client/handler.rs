use rmcp::{
    ClientHandler,
    model::{ClientCapabilities, ClientInfo, Implementation},
};

/// A generic client handler implementing rmcp's ClientHandler trait.
/// All request handling methods (ping, create_message, list_roots, create_elicitation)
/// use safe, robust default implementations that decline unsupported operations.
#[derive(Clone, Debug)]
pub struct GenericClientHandler {
    info: ClientInfo,
}

impl GenericClientHandler {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        let implementation = Implementation::new(name, version);
        let info = ClientInfo::new(ClientCapabilities::default(), implementation);
        Self { info }
    }
}

impl Default for GenericClientHandler {
    fn default() -> Self {
        Self::new("ChatCMD", env!("CARGO_PKG_VERSION"))
    }
}

impl ClientHandler for GenericClientHandler {
    fn get_info(&self) -> ClientInfo {
        self.info.clone()
    }
}
