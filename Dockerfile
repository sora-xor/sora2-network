FROM nixos/nix

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

RUN nix-env -i git
RUN nix-env -iA nixpkgs.cached-nix-shell
COPY shell.nix /
SHELL ["cached-nix-shell", "/shell.nix", "--run"]
RUN rustc --version
COPY . /workdir
WORKDIR /workdir
RUN cargo build --release
RUN cargo test --release
