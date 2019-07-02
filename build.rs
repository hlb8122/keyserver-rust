fn main() {
    prost_build::compile_protos(
        &[
            "src/proto/addressmetadata.proto",
            "src/proto/paymentrequest.proto",
        ],
        &["src/"],
    )
    .unwrap();
}
