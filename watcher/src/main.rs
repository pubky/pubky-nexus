// TODO: Research if that one is the best approach
// Maybe here we can create an POST endpoint that fakes the event stream
// After with mpsc (Multiple Producer Single Consumer) broadcast the event
// to all consumers. In our case, graph and kv
fn main() {
    println!("Hello, world!");
}
