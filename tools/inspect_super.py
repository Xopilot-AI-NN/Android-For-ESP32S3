#!/usr/bin/env python3
from __future__ import annotations
import argparse, hashlib, struct
from pathlib import Path

LP_RESERVED=4096; GEO_SIZE=4096; META_MAX=65536; HEADER=128; SECTOR=512

def cpio_extract(blob: bytes, wanted: str) -> bytes:
    cur=0
    while cur+110 <= len(blob):
        h=blob[cur:cur+110]
        if h[:6] not in (b'070701',b'070702'): raise ValueError('cpio magic')
        fs=int(h[54:62],16); ns=int(h[94:102],16)
        no=cur+110; do=(no+ns+3)&~3
        name=blob[no:no+ns].split(b'\0',1)[0].decode()
        if name=='TRAILER!!!': break
        if name.lstrip('./')==wanted.lstrip('./'):
            return blob[do:do+fs]
        cur=(do+fs+3)&~3
    raise KeyError(wanted)

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('image'); a=ap.parse_args()
    data=Path(a.image).read_bytes()
    geo=bytearray(data[LP_RESERVED:LP_RESERVED+52])
    assert struct.unpack_from('<I',geo,0)[0]==0x616c4467
    expected=bytes(geo[8:40]); geo[8:40]=bytes(32)
    assert hashlib.sha256(geo).digest()==expected
    off=LP_RESERVED+2*GEO_SIZE
    hdr=bytearray(data[off:off+HEADER])
    assert struct.unpack_from('<I',hdr,0)[0]==0x414c5030
    expected=bytes(hdr[12:44]); hdr[12:44]=bytes(32)
    assert hashlib.sha256(hdr).digest()==expected
    ts=struct.unpack_from('<I',hdr,44)[0]
    tables=data[off+HEADER:off+HEADER+ts]
    assert hashlib.sha256(tables).digest()==data[off+48:off+80]
    desc=[]
    for doff in (80,92,104,116): desc.append(struct.unpack_from('<III',data,off+doff))
    po,pc,ps=desc[0]; eo,ec,es=desc[1]
    parts={}
    for i in range(pc):
        e=tables[po+i*ps:po+(i+1)*ps]
        name=e[:36].split(b'\0',1)[0].decode()
        first,num=struct.unpack_from('<II',e,40)
        assert num==1
        ex=tables[eo+first*es:eo+(first+1)*es]
        sectors,target_type,target_data,target_source=struct.unpack_from('<QIQI',ex,0)
        assert target_type==0 and target_source==0
        parts[name]=(target_data*SECTOR,sectors*SECTOR)
    required={
      'system_a':'framework/zephyr-framework.rhai',
      'vendor_a':'etc/zephyr/vendor_runtime.rhai',
      'product_a':'etc/zephyr/product_runtime.rhai',
      'odm_a':'etc/zephyr/odm_runtime.rhai',
      'system_ext_a':'etc/zephyr/system_ext_runtime.rhai',
    }
    for part,path in required.items():
        start,size=parts[part]
        payload=cpio_extract(data[start:start+size],path)
        print(f'{part:14s} {start//1024//1024:3d} MiB  {path}: {len(payload)} bytes')
    print('OK: Android liblp geometry/header/tables + logical partition CPIO userspace verified')
if __name__=='__main__': main()
