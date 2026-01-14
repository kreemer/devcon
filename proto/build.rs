fn main() {
    prost_build::compile_protos(&["agent.proto"], &["."]).unwrap();
}
