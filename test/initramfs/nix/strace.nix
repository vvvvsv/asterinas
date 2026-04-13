{ autoreconfHook, git, lib, pkgsHostTarget }:
let
  localSrcPath = /root/wsy/strace;
  hasLocalSrc = builtins.pathExists localSrcPath;
  localSrc = builtins.path {
    path = localSrcPath;
    name = "wsy-strace-src";
  };
in if !hasLocalSrc then
  pkgsHostTarget.strace
else
  pkgsHostTarget.strace.overrideAttrs (old: {
    version = "${old.version}-wsy";
    src = localSrc;
    doCheck = false;
    nativeBuildInputs = (old.nativeBuildInputs or [ ])
      ++ [ autoreconfHook git ];
    preAutoreconf = (old.preAutoreconf or "") + ''
      patchShebangs .
      ./bootstrap
    '';
  })
