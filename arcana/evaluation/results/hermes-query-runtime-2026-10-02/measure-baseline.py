import json, subprocess, time, threading, psutil, sys
from pathlib import Path
exe, snapshot, output = sys.argv[1:]
p = subprocess.Popen([exe, "protocol", "--snapshot", snapshot], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
peak = [0]
def monitor():
    proc = psutil.Process(p.pid)
    while p.poll() is None:
        try: peak[0] = max(peak[0], proc.memory_info().rss)
        except psutil.Error: break
        time.sleep(.01)
t = threading.Thread(target=monitor); t.start()
requests = [{"op":"capabilities"},{"op":"stats"},{"op":"resolve_symbol","name":"run_agent","limit":10}]
results=[]
start=time.perf_counter()
for i, request in enumerate(requests):
    request["id"]=i
    before=time.perf_counter(); p.stdin.write(json.dumps(request)+"\n");p.stdin.flush()
    line=p.stdout.readline()
    results.append({"op":request["op"],"seconds":time.perf_counter()-before,"response":json.loads(line) if line else None})
p.stdin.close();p.wait(timeout=180);t.join()
report={"exe":exe,"snapshot":snapshot,"elapsed_seconds":time.perf_counter()-start,"peak_rss_bytes":peak[0],"exit_code":p.returncode,"stderr":p.stderr.read(),"queries":results}
Path(output).write_text(json.dumps(report,indent=2),encoding="utf-8")
print(json.dumps({k:v for k,v in report.items() if k!="queries"}));print(json.dumps([{"op":r["op"],"seconds":r["seconds"]} for r in results]))
