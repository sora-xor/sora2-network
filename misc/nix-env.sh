. ./scripts/partial/helpers.sh;

export LIBCLANG_PATH=`first_ls /nix/store/*clang*-lib/lib/libclang.so*.7 | sed 's,/libclang.*,,'`
export PROTOC=`which protoc`
export OPENSSL_INCLUDE_DIR=`first /nix/store/*openssl*-1.1.1g-dev/`
export OPENSSL_LIBDIR=`first /nix/store/*-openssl-1.1.1g/lib/`
#export PKG_CONFIG_ALLOW_CROSS=1


