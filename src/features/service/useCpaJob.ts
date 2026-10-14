import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { cpaApi, native, type CpaJob } from "./api";
export function useCpaJob() {
  const [job,setJob]=useState<CpaJob>({busy:false,message:"",revision:0,logs:[]});
  const [error,setError]=useState("");
  const [pending,setPending]=useState(false);
  const running=useRef(false);
  const revision=useRef(0);
  useEffect(()=>{
    if(!native)return;
    let cancelled=false;let dispose:(()=>void)|undefined;
    const apply=(next:CpaJob)=>{
      if(cancelled||next.revision<revision.current)return;
      if(next.revision>revision.current)setError("");
      revision.current=next.revision;
      setJob(next);
    };
    void (async()=>{
      const stop=await listen<CpaJob>("cpa-job",event=>apply(event.payload));
      if(cancelled){stop();return;}dispose=stop;apply(await cpaApi.job());
    })().catch(()=>{if(!cancelled)setError("CPA 任务状态读取失败");});
    return ()=>{cancelled=true;dispose?.();};
  },[]);
  const run=async(action:()=>Promise<string>)=>{
    if(job.busy||running.current)return false;
    running.current=true;setPending(true);setError("");
    const started=revision.current;
    try{const result=await action();if(revision.current===started)setError(result);return true;}
    catch(e){if(revision.current===started)setError(typeof e==="string"?e:"CPA 操作未完成，请重新检测");return false;}
    finally{running.current=false;setPending(false);}
  };
  return {update:job.update, logs:job.logs ?? [], feedback:error, busy:job.busy||pending,message:job.busy?job.message:error||job.message,run};
}
