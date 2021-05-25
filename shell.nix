{ nixpkgs ? import <nixpkgs> { } }:
nixpkgs.stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = with nixpkgs; [
    # rustc cargo
    clang
    pkgconfig
  ];
  # dav1d 
  buildInputs = with nixpkgs; [ glibc_multi nasm ninja ];
  LIBCLANG_PATH = "${nixpkgs.llvmPackages.libclang.lib}/lib";
}
