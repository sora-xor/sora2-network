. ./scripts/partial/helpers.sh

export_if_not_exist LIBCLANG_PATH `first_ls /nix/store/*clang*-lib/lib/libclang.so*.7 | sed 's,/libclang.*,,'`
export_if_not_exist PROTOC `first /nix/store/*protobuf*/bin/protoc`
export_if_not_exist OPENSSL_INCLUDE_DIR `first /nix/store/*openssl*-1.1.1g-dev/`
export_if_not_exist OPENSSL_LIBDIR `first /nix/store/*-openssl-1.1.1g/lib/`
#export PKG_CONFIG_ALLOW_CROSS=1


