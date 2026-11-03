"""Synthetic images prove slot placement, CRC, alternate writes and exclusive locking."""
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest
import zlib

SOURCE=Path(__file__).resolve().parents[1]/'provision_watch.py'
SPEC=importlib.util.spec_from_file_location('watch_provision',SOURCE)
P=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(P)


class ProvisionTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)
        self.image=self.root/'test.img'
        data=bytearray(3*1024*1024)
        entry=bytearray(128);entry[:16]=bytes([1])*16
        struct.pack_into('<QQ',entry,32,2048,4095)
        entry[56:64]='swap'.encode('utf-16-le')
        data[1024:1152]=entry
        header=bytearray(512);header[:8]=b'EFI PART'
        struct.pack_into('<III',header,8,0x10000,92,0)
        struct.pack_into('<QQQQ',header,24,1,6143,34,6142)
        struct.pack_into('<QIII',header,72,2,1,128,zlib.crc32(entry))
        struct.pack_into('<I',header,16,zlib.crc32(header[:92]))
        data[512:1024]=header;self.image.write_bytes(data)
        self.device=self.root/'device.json';self.device.write_text(json.dumps({'device_id':'d'*22,'device_credential':'k'*43}))
        self.ca=self.root/'ca.pem';self.ca.write_text('-----BEGIN CERTIFICATE-----\nfixture\n-----END CERTIFICATE-----\n')
    def provision(self):
        P.provision(self.image,self.device,self.ca,'https://gateway.example.org')
    def test_alternating_slots_preserve_every_other_byte(self):
        before=self.image.read_bytes();self.provision();first=self.image.read_bytes()
        at=1048576+(76+1)*4096
        self.assertEqual(first[:at],before[:at]);self.assertEqual(first[at+4096:],before[at+4096:])
        gen=P.generation(first[at:at+4096]);self.assertGreater(gen,0)
        self.provision();second=self.image.read_bytes()
        self.assertEqual(first[at:at+4096],second[at:at+4096])
        self.assertGreater(P.generation(second[at+4096:at+8192]),gen)
    def test_running_server_or_foreign_slot_is_never_overwritten(self):
        with self.image.with_suffix('.img.lock').open('a+b') as lock:
            P.fcntl.flock(lock,P.fcntl.LOCK_EX|P.fcntl.LOCK_NB)
            with self.assertRaisesRegex(ValueError,'in use'):self.provision()
        with self.image.open('r+b') as file:
            file.seek(1048576+77*4096);file.write(b'foreign')
        before=self.image.read_bytes()
        with self.assertRaisesRegex(ValueError,'foreign'):self.provision()
        self.assertEqual(self.image.read_bytes(),before)
    def test_gpt_corruption_and_http_fail_before_modification(self):
        with self.assertRaisesRegex(ValueError,'HTTPS'):P.provision(self.image,self.device,self.ca,'http://gateway.example.org')
        with self.image.open('r+b') as file:file.seek(540);file.write(b'bad')
        before=self.image.read_bytes()
        with self.assertRaisesRegex(ValueError,'CRC'):self.provision()
        self.assertEqual(self.image.read_bytes(),before)


if __name__=='__main__':unittest.main()
