fn main() {
    prost_build::compile_protos(
        &[
            "src/proto/addressmetadata.proto",
            "src/proto/paymentrequest.proto",
            "src/proto/s2s.proto",
        ],
        &["src/"],
    )
    .unwrap();
}
