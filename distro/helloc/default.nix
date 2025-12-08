{ pkgs ? import <nixpkgs> { } }:

pkgs.stdenv.mkDerivation {
  pname = "helloc";
  version = "1.0";

  src = ./.;

  buildInputs = [ pkgs.gcc ];

  buildPhase = ''
    gcc -o helloc helloc.c
  '';

  installPhase = ''
    mkdir -p $out/bin
    cp helloc $out/bin/
  '';
}
