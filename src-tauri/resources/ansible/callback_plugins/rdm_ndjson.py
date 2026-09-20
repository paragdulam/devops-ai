"""NDJSON stdout callback for Remote Dev Machine.

Prints one flushed JSON object per task-lifecycle event so the Rust side can
stream real-time progress (rental://{id}/log and .../step) instead of
waiting for a single JSON blob at process exit, which is what the built-in
`ANSIBLE_STDOUT_CALLBACK=json` callback does. Uses only
`ansible.plugins.callback.CallbackBase`, part of `ansible-core` itself --
no extra pip package (e.g. ansible-runner) required.
"""

import json
import sys

from ansible.plugins.callback import CallbackBase

DOCUMENTATION = """
    name: rdm_ndjson
    type: stdout
    short_description: NDJSON event stream for the Remote Dev Machine app
"""


class CallbackModule(CallbackBase):
    CALLBACK_VERSION = 2.0
    CALLBACK_TYPE = "stdout"
    CALLBACK_NAME = "rdm_ndjson"

    def _emit(self, event, task=None, result=None):
        tags = []
        name = ""
        if task is not None:
            name = task.get_name()
            try:
                tags = list(task.tags)
            except Exception:
                tags = []

        payload = {"event": event, "task": name, "tags": tags}
        if result is not None:
            payload["host"] = result._host.get_name() if result._host else None
            if event == "failed":
                payload["msg"] = result._result.get("msg", "")

        sys.stdout.write(json.dumps(payload) + "\n")
        sys.stdout.flush()

    def v2_playbook_on_task_start(self, task, is_conditional):
        self._emit("start", task=task)

    def v2_runner_on_ok(self, result):
        self._emit("ok", task=result._task, result=result)

    def v2_runner_on_failed(self, result, ignore_errors=False):
        self._emit("failed", task=result._task, result=result)

    def v2_runner_on_skipped(self, result):
        self._emit("skipped", task=result._task, result=result)

    def v2_playbook_on_stats(self, stats):
        hosts = sorted(stats.processed.keys())
        summary = {h: stats.summarize(h) for h in hosts}
        sys.stdout.write(json.dumps({"event": "stats", "summary": summary}) + "\n")
        sys.stdout.flush()
