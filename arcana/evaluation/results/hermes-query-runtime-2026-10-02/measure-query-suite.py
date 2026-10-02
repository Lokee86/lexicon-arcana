import json, subprocess, time, threading, psutil, sys, statistics
from pathlib import Path
exe, snapshot, output = sys.argv[1:]
p = subprocess.Popen([exe,"protocol","--snapshot",snapshot],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding="utf-8")
peak=[0]
def monitor():
    proc=psutil.Process(p.pid)
    while p.poll() is None:
        try: peak[0]=max(peak[0],proc.memory_info().rss)
        except psutil.Error:break
        time.sleep(.005)
t=threading.Thread(target=monitor);t.start()
results=[]
def query(request):
    request={**request,"id":len(results)}
    before=time.perf_counter();p.stdin.write(json.dumps(request)+"\n");p.stdin.flush()
    timer=threading.Timer(30,p.kill);timer.start()
    line=p.stdout.readline();timer.cancel()
    if not line:raise RuntimeError(p.stderr.read())
    response=json.loads(line)
    result={"op":request["op"],"args":request,"seconds":time.perf_counter()-before,"ok":response["ok"]}
    results.append(result);print(json.dumps(result),flush=True)
    return response
query({"op":"capabilities"})
query({"op":"stats"})
page=query({"op":"list_nodes","limit":10})
node=page["result"]["nodes"][0]
for i in range(100):query({"op":"resolve_symbol","name":node["name"],"limit":10})
for request in [
 {"op":"search_nodes","query":"run_agent","limit":10},
 {"op":"search_nodes","query":"a","limit":10},
 {"op":"unresolved","limit":10},
 {"op":"unresolved","node_id":node["node_id"],"limit":10},
 {"op":"neighbors","node_id":node["node_id"],"direction":"outgoing","limit":10},
 {"op":"export_graph","limit":10},
 {"op":"architecture_summary","path_prefix":"hermes_cli","min_community_size":1,"limit":10},
 {"op":"diff","other_snapshot":snapshot,"limit":10},
]:query(request)
p.stdin.close();p.wait(timeout=30);t.join()
latencies=sorted(r["seconds"] for r in results if r["op"]=="resolve_symbol")
report={"executable":exe,"snapshot":snapshot,"cache_condition":"uncontrolled warm filesystem cache; fresh process","peak_rss_bytes":peak[0],"exact_lookup_p95_seconds":latencies[94],"exit_code":p.returncode,"stderr":p.stderr.read(),"queries":results}
Path(output).write_text(json.dumps(report,indent=2),encoding="utf-8")
print(json.dumps({k:v for k,v in report.items() if k!="queries"}))
