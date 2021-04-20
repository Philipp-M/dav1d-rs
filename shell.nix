{ nixpkgs ? import <nixpkgs> { } }:
nixpkgs.stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = with nixpkgs; [
    # rustc cargo
    clang
    pkgconfig
  ];
  buildInputs = with nixpkgs; [ glibc_multi dav1d nasm ninja ];
  LIBCLANG_PATH = "${nixpkgs.llvmPackages.libclang}/lib";
}
