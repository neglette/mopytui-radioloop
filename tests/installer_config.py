import importlib.util
from pathlib import Path
import tomllib
import unittest
from unittest.mock import patch, MagicMock
import socket
spec=importlib.util.spec_from_file_location('installer',Path(__file__).resolve().parents[1]/'install-mpc-gateway.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class ConfigTest(unittest.TestCase):
    def test_only_mpd_port_changes(self):
        original='[http]\nport = 6680\n[mpd]\n# retain comment\nport = 6600\nhostname = 127.0.0.1\n[file]\nmedia_dirs = /music\n'
        changed=module.set_ini_port(original,6602)
        self.assertEqual(changed,original.replace('port = 6600','port = 6602'))
        self.assertEqual(module.set_ini_port(changed,6602),changed)
    def test_missing_sections_and_keys(self):
        self.assertIn('[mpd]\nport = 6602',module.set_ini_port('[http]\nport = 6680\n',6602))
        self.assertEqual(tomllib.loads(module.set_tui_port('[radio]\nrotation_min = 23\n',6602)),{'mpd_port':6602,'radio':{'rotation_min':23}})
    def test_radio_settings_survive(self):
        original='mpd_port = 6600\nhost = "127.0.0.1"\n[radio]\nservers = ["https://example.test"]\n'
        self.assertEqual(module.set_tui_port(original,6602),original.replace('6600','6602'))
    def test_custom_ports_and_duplicate_sections_rejected(self):
        with self.assertRaises(ValueError): module.set_tui_port('mpd_port = 7700',6602)
        with self.assertRaises(ValueError): module.set_ini_port('[mpd]\nport=6600\n[mpd]\nport=6610',6602)
class PortTest(unittest.TestCase):
    def test_reuseaddr_is_set_before_bind_without_reuseport(self):
        sock=MagicMock()
        with patch.object(module.socket, 'socket') as factory:
            factory.return_value.__enter__.return_value=sock
            module.check_gateway_port(6600)
        self.assertEqual([call[0] for call in sock.mock_calls], ['setsockopt','bind','listen'])
        sock.setsockopt.assert_called_once_with(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
    def test_busy_listener_still_fails(self):
        sock=MagicMock();sock.bind.side_effect=OSError(98,'Address already in use')
        with patch.object(module.socket, 'socket') as factory:
            factory.return_value.__enter__.return_value=sock
            with self.assertRaisesRegex(RuntimeError,'Cannot listen.*98'):
                module.check_gateway_port()
if __name__=='__main__':unittest.main()
