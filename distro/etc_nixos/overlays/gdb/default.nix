self: super:

{
  gdb = super.gdb.overrideAttrs (oldAttrs: {
    patches = (oldAttrs.patches or [ ]) ++ [
      ./0001-Bypass-reading-x86-debug-registers.patch
      ./0002-Bypass-__WALL-in-wait.patch
    ];
  });
}
