#!/usr/bin/env python3
"""L2c NDJSON 结构判据（QA 资产）。

用法：
  ndjson_check.py one <json-stderr> <text-stderr> <src>
  ndjson_check.py two <json-stderr> <text-stderr> <src>
  ndjson_check.py schema <json-stderr>
  ndjson_check.py path <json-stderr> <expected-path> <escape|utf8>
"""
import json
import re
import sys

KEYS = ["code", "severity", "message", "file", "line", "col"]
TEXT_RE = re.compile(r"^zorcc: (E[0-9]{4}) (.*):([0-9]+):([0-9]+): (.*)$")


def fail(msg):
    print("FAIL: " + msg)
    sys.exit(1)


def json_lines(path):
    data = open(path, "rb").read()
    if not data:
        fail("JSON stderr 为空")
    if data.startswith(b"\xef\xbb\xbf"):
        fail("JSON stderr 带 BOM")
    if b"\r" in data:
        fail("JSON stderr 含 CR")
    if not data.endswith(b"\n"):
        fail("JSON stderr 末行无 LF")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as e:
        fail("JSON stderr 非 UTF-8: %s" % e)
    raw = text.split("\n")
    if raw and raw[-1] == "":
        raw.pop()
    out = []
    for line in raw:
        try:
            obj = json.loads(line)
        except json.JSONDecodeError as e:
            fail("JSON 解析失败: %s" % e)
        if list(obj.keys()) != KEYS:
            fail("键序/键集不符: %r" % list(obj.keys()))
        if obj["severity"] != "error":
            fail("severity 非 error: %r" % obj["severity"])
        if not isinstance(obj["code"], str) or not re.fullmatch(r"E[0-9]{4}", obj["code"]):
            fail("code 非法: %r" % obj["code"])
        if not isinstance(obj["file"], str) or not obj["file"]:
            fail("file 非字符串/为空")
        for k in ("line", "col"):
            if not isinstance(obj[k], int) or obj[k] < 1:
                fail("%s 非正整数" % k)
        if not isinstance(obj["message"], str) or not obj["message"]:
            fail("message 非字符串/为空")
        out.append(obj)
    return out


def text_lines(path):
    data = open(path, "rb").read()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as e:
        fail("文本 stderr 非 UTF-8: %s" % e)
    out = []
    for line in text.splitlines():
        if not line:
            continue
        m = TEXT_RE.match(line)
        if not m:
            fail("文本诊断不匹配: %r" % line)
        out.append({
            "code": m.group(1), "file": m.group(2),
            "line": int(m.group(3)), "col": int(m.group(4)),
            "message": m.group(5), "severity": "error",
        })
    return out


def cmp_objs(js, tx):
    if len(js) != len(tx):
        fail("JSON 行数 %d != 文本诊断 %d" % (len(js), len(tx)))
    for i, (a, b) in enumerate(zip(js, tx)):
        if a["code"] != b["code"] or a["file"] != b["file"] or a["line"] != b["line"] or a["col"] != b["col"] or a["message"] != b["message"]:
            fail("第 %d 条 JSON 与文本诊断不一致" % (i + 1))


def main():
    if len(sys.argv) < 3:
        fail("参数不足")
    mode = sys.argv[1]
    if mode in ("one", "two"):
        if len(sys.argv) != 5:
            fail("参数不足")
        js = json_lines(sys.argv[2])
        tx = text_lines(sys.argv[3])
        want = 1 if mode == "one" else 2
        if len(js) != want:
            fail("%s 期望 %d 条 JSON，实得 %d" % (mode, want, len(js)))
        cmp_objs(js, tx)
    elif mode == "schema":
        js = json_lines(sys.argv[2])
        if len(js) != 1:
            fail("schema 期望恰 1 条 JSON")
    elif mode == "path":
        if len(sys.argv) != 5:
            fail("参数不足")
        js = json_lines(sys.argv[2])
        if len(js) != 1:
            fail("path 期望恰 1 条 JSON")
        if js[0]["file"] != sys.argv[3]:
            fail("file 字段不符: %r" % js[0]["file"])
        if sys.argv[4] not in ("escape", "utf8"):
            fail("path 子模式非法")
    else:
        fail("mode 非法: %s" % mode)
    print("OK")


if __name__ == "__main__":
    main()
