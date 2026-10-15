import { useEffect, useState } from 'react';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

type Status = { listening: boolean; status: string; permission: boolean; inputPermission?: boolean; accessibilityPermission?: boolean; screenPermission?: boolean; collision?: string; platform: string; version: string; diagnostics?: number[] | null };
export function App() {
  const [status, setStatus] = useState<Status>({ listening: false, status: '已暂停', permission: false, platform: navigator.platform.toLowerCase().includes('mac') ? 'macos' : 'windows', version: '0.1.0' });
  const [error, setError] = useState(''); const [busy, setBusy] = useState(false);
  const [clicks, setClicks] = useState(0);
  const desktop = isTauri(); const mac = status.platform === 'macos';
  const refresh = async () => { if (desktop) setStatus(await invoke<Status>('get_status')); };
  useEffect(() => {
    let cancelled = false;
    const poll = () => { if (!cancelled && !document.hidden) void refresh().catch(e => setError(String(e))); };
    poll(); const timer = setInterval(poll, 1000);
    return () => { cancelled = true; clearInterval(timer); };
  }, []);
  const action = async (fn: () => Promise<unknown>) => { setBusy(true); setError(''); try { await fn(); await refresh(); } catch (e) { setError(String(e)); } finally { setBusy(false); } };
  return <main>
    <header><a className="brand" href="#about"><img className="brand-mark" src="/app-logo.png" alt="MeowBonk 小猫图标"/> MeowBonk</a><span className="version">v{status.version} · {mac ? 'macOS' : 'Windows'}</span></header>
    <section className="hero">
      <div className="hero-copy"><p className="eyebrow">A LITTLE CAT. A BIG ATTITUDE.</p><h1>王之蔑视<span>区区一个按钮。</span></h1><p className="intro">它可以不屑。<br/>也可以跑过去，替你撞一下。</p><div className="status"><i className={status.listening ? 'live' : ''}/>{status.listening ? '已就位，等待召唤' : '正在冷眼旁观'}</div></div>
      <div className="portrait"><div className="halo"/><img src="/cat-standing.png" alt="半眯着眼的黑白奶牛猫"/><span className="caption">「 就这？ 」</span></div>
    </section>
    <section className="controls" aria-label="召唤控制"><div><h2>按住，然后点一下</h2><p>在主显示器上，按住 <kbd>{mac ? '⌥ Option' : 'Alt'}</kbd> + <kbd>Shift</kbd> 并点击目标。</p><small>组合点击只召唤小猫，不触发目标；普通点击照常。启用或暂停状态会在下次启动时恢复。</small></div><button disabled={busy || !desktop} className={status.listening ? 'secondary' : 'primary'} onClick={() => void action(() => invoke('set_listening', { enabled: !status.listening }))}>{busy ? '处理中…' : status.listening ? '暂停召唤' : '启用召唤'}</button></section>
    {!desktop && <p className="notice">当前是界面预览。请启动桌面应用使用全局输入与小猫窗口。</p>}
    {desktop && <p className="notice" role="status">{status.status}{mac && !status.permission ? `。待授权：${[!status.inputPermission && '输入监控', !status.accessibilityPermission && '辅助功能'].filter(Boolean).join('、')}。请按系统提示授权后重新启用。` : ''}</p>}
    {desktop && mac && <section className="controls"><div><h2>让点击位置也被撞一下</h2><p>在触发时截取点击点附近画面，与小猫撞击同时挤压、震动和回弹。画面只留在内存，不保存或上传；截图不可用时本次撞击不播放。</p><small>{status.screenPermission ? '屏幕录制已允许' : '需要在系统设置中允许屏幕录制；授权后如系统提示，请重启应用。'} · {status.collision}</small></div><button disabled={busy || status.screenPermission} onClick={() => void action(() => invoke('enable_screen_capture'))}>{status.screenPermission ? '已允许' : '允许屏幕录制'}</button></section>}
    {error && <p className="error" role="alert">{error}</p>}
    {status.diagnostics && <p className="notice">调试计数（仅本次运行）：左键 {status.diagnostics[0]} · Option {status.diagnostics[1]} · Shift {status.diagnostics[2]} · 匹配 {status.diagnostics[3]} · 屏外 {status.diagnostics[4]} · 已发送 {status.diagnostics[5]}</p>}
    <section className="controls"><div><h2>试试这个按钮</h2><p>普通点击增加计数；启用后，组合点击只让小猫撞过来。</p></div><button onClick={() => setClicks(value => value + 1)}>测试按钮 · {clicks}</button></section>
    <section className="details"><div><span className="number">01</span><h3>先让它动一下</h3><p>演示无需输入权限，但需要屏幕录制权限；点击位置就是撞击和果冻变形的位置。</p><button disabled={busy || !desktop} onClick={event => { const rect = event.currentTarget.getBoundingClientRect(); void action(() => invoke('demo', {point:{x:rect.x+rect.width/2,y:rect.y+rect.height/2}})); }}>演示撞击 <span>↗</span></button></div><div><span className="number">02</span><h3>一切都在小猫那里</h3><p>右击静止的小猫，打开设置、应用信息或退出。关闭本窗口后，小猫继续留在桌面。</p><button disabled={!desktop} onClick={() => void action(() => getCurrentWindow().hide())}>回到桌面 <span>↗</span></button></div></section>
    <footer id="about"><div><strong>MeowBonk · 王之蔑视</strong><p>本地运行 · 不记录键入内容 · GitHub 分发</p><p>点击哪里，就撞哪里，小猫和局部画面一起变形。多屏与 Windows 画面捕获待完善。</p></div><button className="text-button" disabled={!desktop} onClick={() => void action(() => invoke('quit'))}>退出应用</button></footer>
  </main>;
}
