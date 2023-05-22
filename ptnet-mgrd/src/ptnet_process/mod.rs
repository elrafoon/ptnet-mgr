mod nodescan;
mod persist;
mod fwu;

pub use nodescan::*;
pub use persist::*;
pub use fwu::*;

use async_trait::async_trait;

#[async_trait]
pub trait PtNetProcess {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    //async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    //fn start(&mut self) -> JoinHandle<()>;
    //fn start(&mut self) -> BoxFuture<'static, Result<(), Box<dyn std::error::Error>>>;
}
