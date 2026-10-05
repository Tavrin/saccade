#!/usr/bin/env python3
"""Offline boundary regressions; mocked replay is not live Vulkan qualification."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace as NS

WORKER = Path(__file__).with_name('renderdoc-worker.py')
spec = importlib.util.spec_from_file_location('worker', WORKER)
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)

class BoundaryTests(unittest.TestCase):
    def test_capture_fifo_rejected_before_open(self):
        with tempfile.TemporaryDirectory() as d:
            fifo = Path(d) / 'fifo'
            os.mkfifo(fifo)
            code = 'import runpy,sys; w=runpy.run_path(sys.argv[1]); w["file_hash"](sys.argv[2])'
            try:
                p = subprocess.run([sys.executable, '-c', code, str(WORKER), str(fifo)], capture_output=True, timeout=2)
            except subprocess.TimeoutExpired:
                self.fail('FIFO hashing blocked')
            self.assertNotEqual(p.returncode, 0)
            self.assertIn(b'regular file', p.stderr)

    def test_native_worker_deadline_terminates_stall(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d, 'renderdoc.py').write_text('import time\ntime.sleep(30)\n')
            capture = Path(d, 'capture.rdc'); capture.write_bytes(b'capture')
            env = dict(os.environ, PYTHONPATH=d)
            p = subprocess.run([sys.executable, str(WORKER), str(capture), '--out', str(Path(d,'out')), '--timeout-seconds', '0.2'], env=env, capture_output=True, timeout=3)
            self.assertEqual(p.returncode, 2)
            self.assertIn('deadline', p.stdout.decode())

    def test_clear_never_reads_unrelated_graphics_bindings(self):
        action = NS(children=[], flags=4, eventId=1, customName='clear', copyDestination='different-clear-target')
        class Controller:
            def GetAPIProperties(self): return NS(pipelineType='Vulkan')
            def GetTextures(self): return []
            def GetBuffers(self): return []
            def GetStructuredFile(self): return None
            def GetRootActions(self): return [action]
            def SetFrameEvent(self, *_): pass
            def GetPipelineState(self):
                raise AssertionError('clear inspected unrelated bound target')
            def Shutdown(self): pass
        class Capture:
            def OpenFile(self, *_): return 0
            def LocalReplaySupport(self): return 0
            def OpenCapture(self, *_): return 0, Controller()
            def Shutdown(self): pass
        rd=NS(OpenCaptureFile=Capture, ResultCode=NS(Succeeded=0), ReplaySupport=NS(Supported=0), ReplayOptions=lambda:NS(), ReplayOptimisationLevel=NS(Conservative=0), GraphicsAPI=NS(Vulkan='Vulkan'), ActionFlags=NS(Dispatch=1,Drawcall=2,Clear=4))
        actions=worker.extract(rd, 'capture', Path('.'), 100, False)
        self.assertEqual(actions[0]['kind'], 'clear')
        self.assertEqual(actions[0]['resources'], [])
        self.assertEqual(actions[0]['candidate_inputs'], [])

if __name__ == '__main__':
    unittest.main()
