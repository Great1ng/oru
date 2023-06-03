use std::env;

mod node;

const BOOT_NODE_ADDRESS: &str = "/ip4/34.74.197.69/tcp/80";

#[tokio::main]
async fn main() {
    let port = env::args()
        .nth(1)
        .expect("Expected port")
        .parse()
        .expect("Invalid port");

    let mut node = node::Node::new();
    println!("Node id: {}", node.get_id());

    let mut connection = node
        .connect(BOOT_NODE_ADDRESS, Some(port))
        .await
        .expect("Could not connect to boot node");

    connection.handle().await.expect("Connection failed");
}
