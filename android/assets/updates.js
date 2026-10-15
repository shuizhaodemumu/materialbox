'use strict';
(()=>{
 const dialog=document.getElementById('update-dialog'),el=id=>document.getElementById(id);
 let data={state:'idle',current:'0.4.1'},requested=false;
 const titles={checking:'正在检查更新',available:'发现新版本',current:'已经是最新版本',downloading:'新版本正在路上',verifying:'正在核验安装包',ready:'下载完成，准备安装',error:'暂时没能完成',cancelled:'已取消下载',idle:'检查更新'};
 const bytes=n=>(Number(n||0)/1024/1024).toFixed(1)+' MB';
 function close(){requested=false;dialog.close();}
 function render(value){if(typeof value==='string')try{value=JSON.parse(value);}catch(_){return;}if(!value||typeof value!=='object')return;const previous=data.state;data=value;const state=data.state||'idle',busy=['checking','downloading','verifying'].includes(state);
  el('app-version').textContent=data.current||'0.4.1';el('update-title').textContent=titles[state]||'检查更新';el('update-message').textContent=data.message||'仅在你点击时连接 GitHub，不影响离线转换。';
  el('update-card-status').textContent=state==='downloading'?'新版本下载中 · '+Math.max(0,Math.min(100,Number(data.percent)||0))+'%':state==='ready'?'新版本已下载，点击继续安装。':state==='available'?'发现 '+data.latest+'，点击查看更新。':state==='idle'?'手动连接 GitHub 检查更新，其他功能仍可离线使用。':data.message||titles[state];
  el('check-update').textContent=['downloading','verifying','available','ready'].includes(state)?'查看更新':'检查更新';
  const newer=!!data.latest&&state!=='checking'&&state!=='current'&&state!=='error';el('update-versions').classList.toggle('hidden',!newer);el('update-from').textContent='当前 '+(data.current||'');el('update-to').textContent='新版 '+(data.latest||'');el('update-size').textContent=bytes(data.total);
  el('update-notes').textContent=data.notes||'维护者未填写更新说明。';el('update-notes-wrap').classList.toggle('hidden',!newer||!data.notes);
  const progress=['downloading','verifying','ready'].includes(state);el('update-meter').classList.toggle('hidden',!progress);const p=Math.max(0,Math.min(100,Number(data.percent)||0));el('update-percent').textContent=state==='verifying'?'校验中':p+'%';el('update-bytes').textContent=bytes(data.downloaded)+' / '+bytes(data.total);el('update-progress').setAttribute('aria-valuenow',String(p));el('update-progress').querySelector('i').style.width=p+'%';
  el('update-cancel').classList.toggle('hidden',!['downloading','ready'].includes(state));el('update-cancel').textContent=state==='ready'?'删除安装包':'取消下载';el('update-action').classList.toggle('hidden',busy);el('update-action').textContent=state==='available'?'下载新版本':state==='ready'?'立即安装':state==='current'?'完成':'重新检查';el('update-later').textContent=state==='downloading'?'后台下载':state==='checking'||state==='verifying'?'关闭窗口':'稍后再说';
  if(requested&&!dialog.open)dialog.showModal();if(state==='ready'&&previous!=='ready'&&!dialog.open&&typeof toast==='function')toast('更新已下载，可在关于页继续安装');
 }
 function nativeCall(name){if(window.Native&&typeof window.Native[name]==='function')window.Native[name]();else render({state:'error',current:data.current,message:'请在安卓应用内检查和下载更新。'});}
 el('check-update').onclick=()=>{requested=true;if(['available','downloading','verifying','ready'].includes(data.state)){render(data);return;}render({...data,state:'checking',message:'正在连接 GitHub 检查正式发行版…'});nativeCall('checkUpdate');};
 el('update-action').onclick=()=>{if(data.state==='available')nativeCall('downloadUpdate');else if(data.state==='ready')nativeCall('installUpdate');else if(data.state==='current')close();else{render({...data,state:'checking',message:'正在检查更新…'});nativeCall('checkUpdate');}};
 el('update-cancel').onclick=()=>nativeCall('cancelUpdate');el('update-close').onclick=close;el('update-later').onclick=close;dialog.addEventListener('cancel',()=>{requested=false;});
 const old=window.nativeEvent;window.nativeEvent=(name,value)=>name==='update'?render(value):old(name,value);
 const back=window.goBack;window.goBack=()=>{if(dialog.open){close();return true;}return back();};
 if(window.Native&&typeof window.Native.updateInfo==='function')try{render(JSON.parse(window.Native.updateInfo()));}catch(_){render(data);}else render(data);
})();
