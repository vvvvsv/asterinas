{ pkgs ? import <nixpkgs> { } }:

pkgs.stdenv.mkDerivation {
  pname = "helloc";
  version = "1.0";

  src = ./.;

  dontPatchELF = 1;
  dontPatchShebangs = 1;
  dontStrip = 1;
  dontCheckReferences = 1;
  dontCheckForBrokenSymlinks = 1;
  dontRewriteSymlinks = 1;

  buildInputs = [ pkgs.gcc ];

  buildPhase = ''
    gcc -o helloc helloc.c
  '';

  installPhase = ''
    mkdir -p $out/bin
    cp helloc $out/bin/
  '';
}
