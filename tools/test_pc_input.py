import sys
from pathlib import Path
from collections import deque
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from usb_block_server import InputDelivery, deliver_input

class Port:
    def __init__(self):self.data=bytearray()
    def write(self,data):self.data.extend(data);return len(data)
    def flush(self):pass
class Bridge:
    def __init__(self,*commands):self.commands=deque(commands)
    def pop_command(self):return self.commands.popleft() if self.commands else None
class InputTests(unittest.TestCase):
    def test_new_firmware_disables_unsolicited_input_legacy_does_not(self):
        delivery=InputDelivery()
        self.assertFalse(delivery.polling)
        self.assertFalse(delivery.observe('@ZWUI|M3|1'))
        self.assertTrue(delivery.observe('@ZWUI|INPUT_POLL|1'))
        self.assertTrue(delivery.polling)
        self.assertFalse(InputDelivery().polling) # reset/old firmware
    def test_one_command_per_poll_and_explicit_end_marker(self):
        port=Port();bridge=Bridge('@ZWIN|POWER','@ZWIN|NORMAL')
        deliver_input(port,bridge,True)
        self.assertEqual(port.data,b'@ZWIN|POWER\r\n@ZBLK|IDLE\r\n')
        self.assertEqual(len(bridge.commands),1)
        port.data.clear();deliver_input(port,bridge,True)
        self.assertEqual(port.data,b'@ZWIN|NORMAL\r\n@ZBLK|IDLE\r\n')
    def test_empty_or_booting_poll_cannot_consume_queued_commands(self):
        port=Port();bridge=Bridge('@ZWIN|POWER')
        deliver_input(port,bridge,False)
        self.assertEqual(port.data,b'@ZBLK|IDLE\r\n')
        self.assertEqual(len(bridge.commands),1)
        port.data.clear();deliver_input(port,None,True)
        self.assertEqual(port.data,b'@ZBLK|IDLE\r\n')
if __name__ == '__main__':unittest.main()
