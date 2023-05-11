mod nodescan;
mod persist;

pub use nodescan::*;
pub use persist::*;

use async_trait::async_trait;

#[async_trait]
pub trait PtNetProcess {
    async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    //async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>>;
    //fn start(&mut self) -> JoinHandle<()>;
    //fn start(&mut self) -> BoxFuture<'static, Result<(), Box<dyn std::error::Error>>>;
}
