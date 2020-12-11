{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs; [ protobuf llvm clang pkgconfig openssl ];
}
