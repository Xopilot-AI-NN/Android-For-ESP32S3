# Streaming JPEG decoder

ChaN TJpgDec R0.03, distributed by [Bodmer/TJpg_Decoder](https://github.com/Bodmer/TJpg_Decoder)
at commit 71bfc2607b6963ee3334ff6f601345c5d2b7a8da. Original copyright/license is preserved
at the top of tjpgd.c. The local bridge is project code; decoder sources are unchanged.
JPEG input is read through bounded storage callbacks, RGB565 blocks are scaled into a 240×200 preview.
