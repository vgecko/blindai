# CI tests

# To build this image you should first build the dev-env image from the dockerfile in the .devcontainer folder.

# The goal is to handle the less likey and expensive file changes (dependency changes essentially) at the beginning of the build.
# This allow us to reuse the dependencies and ressources needed in the majority of cases

FROM blindaiv2-dev

# build Rust dependencies
RUN cargo new blindaiv2
WORKDIR /blindaiv2
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY tar-rs-sgx tar-rs-sgx
COPY tract tract
COPY ring-fortanix ring-fortanix

# generate tests onnx models and inputs
COPY tests tests
RUN cd tests && cd mobilenet && bash ./setup.sh

# compile Rust sources
COPY src src
COPY host_server.pem host_server.key ./
RUN cargo test --release --no-run

# unit tests
CMD cargo test --release