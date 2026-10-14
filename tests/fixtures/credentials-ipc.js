// Synthetic IPC only. Never connects to a management service or reads user files.
window.isTauri = true;
localStorage.setItem('focus', 'credentials');
const callbacks = new Map();
const listeners = new Map();
let callbackId = 0;
const row = (id, enabled = false) => ({
  id: `${id}.json:idx`, fileName: `${id}.json`, name: `${id}@example.invalid`, brand: 'openai',
  provider: 'codex', enabled, editable: true, deletable: true, deleteToken: `token-${id}`,
  supported: true, five: null, week: null, resetFive: '', resetWeek: '', quotaAt: null, quotaError: null,
});
window.__testSnapshot = {
  connected: true, saved: true, port: 8317, busy: false, stale: false, error: null, revision: 1,
  credentials: [row('stopped'), {...row('runtime'), deletable: false, editable: false, fileName: ''}],
};
window.__testCalls = [];
window.__testPublish = () => {
  window.__testSnapshot.revision++;
  for (const id of listeners.get('credentials-changed') || [])
    callbacks.get(id)?.({event: 'credentials-changed', payload: structuredClone(window.__testSnapshot)});
};
window.__TAURI_INTERNALS__ = {
  metadata: {currentWindow:{label:'main'}, currentWebview:{label:'main',windowLabel:'main'}},
  transformCallback: cb => { callbacks.set(++callbackId, cb); return callbackId; },
  unregisterCallback: id => callbacks.delete(id),
  invoke: async (cmd, args) => {
    if (cmd === 'plugin:event|listen') {
      const values = listeners.get(args.event) || [];
      values.push(args.handler); listeners.set(args.event, values); return args.handler;
    }
    if (cmd === 'plugin:event|unlisten') {
      listeners.set(args.event, (listeners.get(args.event) || []).filter(id => id !== args.eventId)); return;
    }
    if (cmd === 'discover_services') return {services:[],checkedAt:0,statusKnown:true,warning:null};
    if (cmd === 'set_appearance') return;
    if (cmd === 'cpa_job') return {busy:false,message:'',revision:0};
    if (cmd === 'probe_models') return [];
    if (!['management_snapshot','credential_toggle','credential_import','credential_delete','quota_refresh','credentials_list','credentials_toggle','credentials_delete','copy_client_key'].includes(cmd))
      throw new Error(`Unexpected mock command: ${cmd}`);
    const s = window.__testSnapshot;
    if (cmd !== 'management_snapshot') {
      window.__testCalls.push({cmd, args});
      s.busy = true; s.error = null; window.__testPublish();
      await new Promise(resolve => setTimeout(resolve, 180));
      if (window.__testNextError) {
        s.error = window.__testNextError; window.__testNextError = null;
        s.stale = Boolean(window.__testStale);
      } else if (cmd === 'copy_client_key') {
        window.__testKeyCopies = (window.__testKeyCopies || 0) + 1;
      } else if (cmd === 'credential_toggle') {
        const r = s.credentials.find(r => r.id === args.id);
        if (!r || r.enabled !== args.expected) s.error = '凭证已变化';
        else { r.enabled = args.enabled; r.deleteToken += '-changed'; }
      } else if (cmd === 'credentials_toggle' || cmd === 'credentials_delete') {
        const targets = args.targets;
        if (targets.some(t => !s.credentials.some(r => r.id === t.id && r.deleteToken === t.deleteToken && r.enabled === t.enabled))) s.error = '凭证已变化，请重新选择';
        else if (cmd === 'credentials_delete') s.credentials = s.credentials.filter(r => !targets.some(t => t.id === r.id));
        else for (const r of s.credentials.filter(r => targets.some(t => t.id === r.id))) { r.enabled = args.enabled; r.deleteToken += '-changed'; }
      } else if (cmd === 'quota_refresh') {
        for (const r of s.credentials.filter(r => args.id === null || r.id === args.id)) {r.five = 73; r.quotaAt = 100;}
      } else if (cmd === 'credential_import') {
        let parsed;
        try { parsed = JSON.parse(args.contents); } catch { s.error = 'JSON 格式无效，请检查原文件；内容未上传'; }
        if (parsed) {
          const name = args.name || 'codex-pasted@example.invalid.json';
          if (s.credentials.some(r => r.fileName.toLowerCase() === name.toLowerCase())) s.error = '已存在同名凭证，未覆盖；请核对已导入账号，或重命名文件后选择导入';
          else s.credentials.push({...row(name.replace(/\.json$/i, '')), name: parsed.email || name.replace(/\.json$/i, '')+'@example.invalid', fileName:name});
        }
      } else if (cmd === 'credential_delete') {
        s.credentials = s.credentials.filter(r => r.id !== args.id);
      } else if (cmd === 'credentials_list') { s.stale = false; }
      s.busy = false; window.__testPublish();
    }
    return structuredClone(s);
  },
};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: (_event, id) => callbacks.delete(id)};
