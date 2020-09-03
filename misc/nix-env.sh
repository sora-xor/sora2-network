

export LIBCLANG_PATH=`ls /nix/store/*clang*-lib/lib/libclang.so*.7 | head -n 1 | sed 's,/libclang.*,,'`
export PROTOC=`which protoc`
export OPENSSL_INCLUDE_DIR=`echo /nix/store/*openssl*-1.1.1g-dev/ | fmt -w 1 | head -n 1`
export OPENSSL_LIBDIR=`echo /nix/store/*-openssl-1.1.1g/lib/ | fmt -w 1 | head -n 1`
#export PKG_CONFIG_ALLOW_CROSS=1


