"""Boundary tests using original toy tools; no contest data required."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

BRIDGE = Path(__file__).parents[2] / "crates/nora-lifemate/python/bridge.py"
FAKE_ENV = '''
class FakeEnv:
    def __init__(self): self.calls=[]
    def validate(self,name,args): return {"ok":name=="toy.set" and isinstance(args.get("x"),int),"error":None}
    def call(self,name,args): self.calls.append({"name":name,"arguments":args});return {"ok":True,"done":True}
    def get_tool_calls(self): return self.calls
    def get_state_hash(self): return "a"*64
def ToolEnv(split,sample_id,root): return FakeEnv()
'''


class BridgeTests(unittest.TestCase):
    def transact(self, commands):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / "train/zaic_tools"
            package.mkdir(parents=True)
            (package / "__init__.py").write_text(FAKE_ENV, encoding="utf-8")
            result = subprocess.run([sys.executable, "-I", "-B", "-u", str(BRIDGE), str(root), "train"],
                                    input="".join(json.dumps(c) + "\n" for c in commands), text=True,
                                    encoding="utf-8", capture_output=True, timeout=10)
            return result, [json.loads(line) for line in result.stdout.splitlines()]

    def test_only_executed_calls_enter_authoritative_receipt(self):
        result, responses = self.transact([{"op": "begin", "id": "toy_1"},
            {"op": "call", "name": "toy.set", "arguments": {"x": 1}},
            {"op": "call", "name": "toy.set", "arguments": {"x": "wrong type"}},
            {"op": "finish"}])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(responses[1]["executed"])
        self.assertFalse(responses[2]["executed"])
        self.assertEqual(responses[3], {"tool_calls": [{"name": "toy.set", "arguments": {"x": 1}}], "state_hash": "a" * 64})

    def test_session_cannot_be_reset_or_abandoned(self):
        for commands in ([{"op": "begin", "id": "toy_1"}, {"op": "finish"}, {"op": "begin", "id": "toy_1"}],
                         [{"op": "begin", "id": "toy_1"}, {"op": "begin", "id": "toy_2"}]):
            result, responses = self.transact(commands)
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertNotIn("state", responses[-1])

    def test_state_access_and_id_path_traversal_are_not_protocol_operations(self):
        for commands in ([{"op": "begin", "id": "toy_1"}, {"op": "get_state"}],
                         [{"op": "begin", "id": "../state.json"}]):
            result, _ = self.transact(commands)
            self.assertEqual(result.returncode, 2, result.stderr)


if __name__ == "__main__":
    unittest.main()
