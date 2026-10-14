"""Synthetic lifecycle IPC only: no real CPA process, repository or key access."""
import os
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
url=os.environ.get('MODEL_LENS_TEST_URL','http://127.0.0.1:1422')
fixture=Path(__file__).parent/'fixtures/credentials-ipc.js'
setup=r"""
window.service={services:[{executable:'/synthetic/CLIProxy/.dev/cpa',directory:'/synthetic/CLIProxy',workingDirectory:'/synthetic/CLIProxy/.dev',lastSeen:1,pid:123}],checkedAt:1,statusKnown:true,warning:null};
window.job={busy:false,message:'',revision:0,logs:[]};
window.emit=(event,payload)=>{for(const id of listeners.get(event)||[])callbacks.get(id)?.({event,payload:structuredClone(payload)});};
window.report=(busy,message)=>{job={busy,message,revision:job.revision+1,logs:[...job.logs,{id:job.revision+1,at:100,message}]};emit('cpa-job',job);};
window.finish=(message)=>{if(window.nextRunning!==undefined){service.services[0].pid=nextRunning?456:null;window.nextRunning=undefined;}report(false,message);window.resolveJob?.(message);emit('refresh-service',null);};
const originalInvoke=__TAURI_INTERNALS__.invoke;
__TAURI_INTERNALS__.invoke=async(cmd,args)=>{
 if(cmd==='discover_services'){__testCalls.push({cmd,args});return structuredClone(service);}
 if(cmd==='cpa_job')return structuredClone(job);
 if(cmd==='cpa_choose_directory')return '/synthetic/selected/CLIProxy';
 if(cmd==='cpa_set_running'||cmd==='cpa_install'){
  __testCalls.push({cmd,args});
  if(cmd==='cpa_install'&&service.services.length)throw '已发现 CLIProxy 进程或安装目录，请选择已有服务启动；未重复安装';
  if(cmd==='cpa_set_running')window.nextRunning=args.running;
  report(true,cmd==='cpa_install'?'正在编译新安装的 CPA…':args.running?'正在启动 CPA…':'正在关闭 CPA…');
  return new Promise(resolve=>{window.resolveJob=resolve;});
 }
 return originalInvoke(cmd,args);
};
"""
with sync_playwright() as p:
 for engine in [p.webkit,p.chromium]:
  browser=engine.launch(headless=True)
  context=browser.new_context(viewport={'width':1120,'height':760})
  context.add_init_script(script=fixture.read_text()+'\n'+setup)
  page=context.new_page();errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
  page.goto(url)
  page.locator('.service-status').click()
  dialog=page.get_by_role('dialog',name='本机 CLIProxy')
  restart=dialog.get_by_role('button',name='关闭 CPA',exact=True)
  detect=dialog.get_by_role('button',name='重新检测',exact=True)
  expect(restart).to_be_enabled()
  assert restart.bounding_box()['x']<detect.bounding_box()['x']
  expect(dialog.locator('.service-location details')).not_to_have_attribute('open','')
  expect(dialog.locator('.service-location dl')).not_to_be_visible()
  dialog.locator('.service-name').click()
  expect(dialog.locator('.service-location dl')).to_be_visible()
  dialog.locator('.service-name').click()
  card=dialog.locator('.service-location').bounding_box()
  summary=dialog.locator('.service-name').bounding_box()
  assert summary['y']-card['y']<=2
  assert card['height']<=52
  header=dialog.locator('.modal-header').bounding_box()
  toolbar=dialog.locator('.service-toolbar').bounding_box()
  assert 18<=toolbar['y']-header['y']-header['height']<=24
  if engine.name=="webkit": page.screenshot(path="/tmp/modellens-service-collapsed.png",full_page=True)
  restart.click()
  expect(dialog.get_by_text('正在关闭 CPA…',exact=True)).to_be_visible()
  expect(detect).to_be_disabled()
  assert page.evaluate("__testCalls.find(c=>c.cmd==='cpa_set_running').args.executable==='/synthetic/CLIProxy/.dev/cpa'")
  dialog.get_by_role('button',name='关闭',exact=True).click()
  page.locator('.service-status').click()
  expect(dialog.get_by_text('正在关闭 CPA…',exact=True)).to_be_visible()
  expect(dialog.get_by_role('button',name='关闭 CPA',exact=True)).to_be_disabled()
  assert page.evaluate("__testCalls.find(c=>c.cmd==='cpa_set_running').args.running===false && __testCalls.find(c=>c.cmd==='cpa_set_running').args.expectedPid===123")
  page.evaluate("finish('CPA 已关闭')")
  start=dialog.get_by_role('button',name='启动 CPA',exact=True)
  expect(start).to_be_enabled();start.click()
  assert page.evaluate("__testCalls.filter(c=>c.cmd==='cpa_set_running').at(-1).args.running===true && __testCalls.filter(c=>c.cmd==='cpa_set_running').at(-1).args.expectedPid===null")
  page.evaluate("finish('CPA 已启动')")
  expect(dialog.get_by_role('button',name='关闭 CPA',exact=True)).to_be_enabled()
  dialog.get_by_role('button',name='安装 CLIProxy',exact=True).click()
  dialog.get_by_label('CLIProxy 安装路径',exact=True).fill('/synthetic/manual/CLIProxy')
  dialog.get_by_role('button',name='安装并启动',exact=True).click()
  expect(dialog.get_by_text('已发现 CLIProxy 进程或安装目录，请选择已有服务启动；未重复安装',exact=True)).to_be_visible()
  page.evaluate("service.services=[];emit('refresh-service',null)")
  expect(dialog.get_by_role('button',name='启动 CPA',exact=True)).to_be_disabled()
  dialog.get_by_role('button',name='选择目录',exact=True).click()
  expect(dialog.get_by_label('CLIProxy 安装路径',exact=True)).to_have_value('/synthetic/selected/CLIProxy')
  dialog.get_by_label('CLIProxy 安装路径',exact=True).fill('/synthetic/manual/CLIProxy')
  dialog.get_by_label('CPA 端口',exact=True).fill('8431')
  dialog.get_by_role('button',name='安装并启动',exact=True).click()
  expect(dialog.get_by_text('正在编译新安装的 CPA…',exact=True)).to_be_visible()
  assert page.evaluate("__testCalls.filter(c=>c.cmd==='cpa_install').at(-1).args.directory==='/synthetic/manual/CLIProxy'")
  assert page.evaluate("__testCalls.filter(c=>c.cmd==='cpa_install').at(-1).args.port===8431")
  expect(dialog.get_by_role('button',name='选择目录',exact=True)).to_be_disabled()
  for width in [920,1120]:
   page.set_viewport_size({'width':width,'height':760})
   assert page.evaluate('document.documentElement.scrollWidth<=innerWidth')
  page.evaluate("finish('CLIProxy 已安装、启动并保存本机管理连接')")
  expect(dialog.get_by_text('CLIProxy 已安装、启动并保存本机管理连接',exact=True)).to_be_visible()
  page.evaluate("report(true,'另一窗口正在更新…')")
  expect(dialog.get_by_text('另一窗口正在更新…',exact=True)).to_be_visible()
  page.evaluate("finish('新版启动失败，已恢复原程序')")
  expect(dialog.get_by_text('新版启动失败，已恢复原程序',exact=True)).to_be_visible()
  expect(dialog.locator('.service-log-line').last).to_contain_text('新版启动失败，已恢复原程序')
  expect(dialog.get_by_role('log')).to_contain_text('CPA 已关闭')
  if engine.name=='webkit':page.screenshot(path='/tmp/modellens-service.png',full_page=True)
  assert not errors, errors
  context.close();browser.close()
  print(engine.name+': startup ordering, shared background progress, duplicate precheck, directory picker/manual entry and install feedback passed')
