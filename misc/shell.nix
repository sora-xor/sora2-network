{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  #buildInputs = with pkgs; [ pkgconfig gmp protobuf llvm clang openssl pkgsi686Linux.glibc.dev ];
  #buildInputs = with pkgs; [ pkgconfig gmp protobuf llvm clang openssl glibc ];
  buildInputs = with pkgs; [ protobuf llvm clang pkgconfig openssl ];
}
