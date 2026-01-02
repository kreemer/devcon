use protobuf_codegen::CodeGen;

fn main() {
    CodeGen::new()
        .inputs(["request.proto"])
        .include("proto")
        .dependency(protobuf_well_known_types::get_dependency(
            "protobuf_well_known_types",
        ))
        .generate_and_compile()
        .unwrap();
}
