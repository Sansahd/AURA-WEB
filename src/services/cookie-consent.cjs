'use strict';

function cookieConsentRejectSource() {
  return `(() => {
    const KEY='__gekkoCookieRejectV2';
    if(window[KEY]?.installed){try{window[KEY].scan?.()}catch{};return{installed:true,reused:true,rejected:Boolean(window[KEY].rejected)}}
    try{Object.defineProperty(Navigator.prototype,'globalPrivacyControl',{configurable:true,get:()=>true})}catch{}
    const rejectLabels=['tout refuser','refuser tout','refuser','je refuse','continuer sans accepter','refuse all','reject all','reject everything','decline all','decline','only necessary','necessary only','essential only','use necessary cookies only','alle ablehnen','alles ablehnen','nur notwendige','nur erforderliche','rechazar todo','rechazar todos','solo necesarias','sólo necesarias','rifiuta tutto','rifiuta tutti','solo necessari','rejeitar tudo','recusar tudo','apenas necessários','alles weigeren','weigeren','alleen noodzakelijke','odrzuć wszystko','odmów wszystkiego','tylko niezbędne'];
    const manageLabels=['gérer mes choix','gerer mes choix','paramétrer','parametrer','personnaliser','manage choices','manage options','cookie settings','customize','einstellungen','auswahl verwalten','configurar','gestionar opciones','impostazioni','gestisci preferenze','gerir preferências','beheer voorkeuren'];
    const directSelectors=['#onetrust-reject-all-handler','#CybotCookiebotDialogBodyButtonDecline','#didomi-notice-disagree-button','[data-testid="uc-deny-all-button"]','[data-testid="uc-deny-all"]','[data-testid="consent-reject-all"]','[data-cookiefirst-action="reject"]','.cky-btn-reject','.cmplz-deny','button[aria-label*="Refuser"]','button[aria-label*="Reject"]'];
    const norm=v=>String(v||'').replace(/\\s+/g,' ').trim().toLocaleLowerCase();
    const labelOf=n=>norm([n?.innerText,n?.textContent,n?.getAttribute?.('aria-label'),n?.getAttribute?.('title'),n?.getAttribute?.('value')].filter(Boolean).join(' '));
    const visible=n=>{try{const s=getComputedStyle(n),r=n.getBoundingClientRect();return s.display!=='none'&&s.visibility!=='hidden'&&Number(s.opacity||1)>0&&r.width>1&&r.height>1}catch{return true}};
    const match=(label,list)=>list.some(x=>label===x||label.startsWith(x+' ')||label.endsWith(' '+x)||label.includes(' '+x+' '));
    const roots=()=>{const out=[document],queue=[document.documentElement].filter(Boolean),seen=new Set(queue);while(queue.length){const node=queue.shift();try{if(node.shadowRoot&&!seen.has(node.shadowRoot)){seen.add(node.shadowRoot);out.push(node.shadowRoot);queue.push(...node.shadowRoot.querySelectorAll('*'))}for(const c of node.querySelectorAll?.('*')||[]){if(!seen.has(c)){seen.add(c);queue.push(c)}}}catch{}}for(const f of document.querySelectorAll('iframe')){try{if(f.contentDocument)out.push(f.contentDocument)}catch{}}return out};
    const click=n=>{if(!n||!visible(n))return false;try{n.click();return true}catch{try{n.dispatchEvent(new MouseEvent('click',{bubbles:true,cancelable:true,view:window}));return true}catch{return false}}};
    const state={installed:true,rejected:false,openedSettings:false,attempts:0,observer:null,timer:null,scan:null};
    const scan=()=>{
      if(state.rejected)return true;
      state.attempts++;
      for(const root of roots()){
        for(const sel of directSelectors){let node=null;try{node=root.querySelector(sel)}catch{}if(node&&click(node)){state.rejected=true;break}}
        if(state.rejected)break;
        let nodes=[];try{nodes=[...root.querySelectorAll('button,[role="button"],a,input[type="button"],input[type="submit"]')]}catch{}
        for(const node of nodes){const label=labelOf(node);if(label&&match(label,rejectLabels)&&click(node)){state.rejected=true;break}}
        if(state.rejected)break;
      }
      if(state.rejected){try{state.observer?.disconnect()}catch{};clearInterval(state.timer);return true}
      if(!state.openedSettings&&state.attempts>=2){
        for(const root of roots()){let nodes=[];try{nodes=[...root.querySelectorAll('button,[role="button"],a')]}catch{}const opener=nodes.find(n=>match(labelOf(n),manageLabels)&&visible(n));if(opener&&click(opener)){state.openedSettings=true;setTimeout(scan,80);break}}
      }
      return false;
    };
    state.scan=scan;window[KEY]=state;scan();
    try{state.observer=new MutationObserver(()=>scan());state.observer.observe(document.documentElement||document,{subtree:true,childList:true,attributes:true,attributeFilter:['aria-label','title','value','class','style']})}catch{}
    state.timer=setInterval(scan,350);
    setTimeout(()=>{try{state.observer?.disconnect()}catch{};clearInterval(state.timer)},20000);
    return{installed:true,rejected:state.rejected};
  })()`;
}

async function installCookieConsentRefusal(webContents) {
  if(!webContents||webContents.isDestroyed?.()) return {ok:false,reason:'webcontents-unavailable'};
  const url=webContents.getURL?.()||'';
  if(!/^https?:\/\//i.test(url)) return {ok:false,reason:'not-web'};
  try {
    const result=await webContents.executeJavaScript(cookieConsentRejectSource(),true);
    return {ok:true,...(result||{})};
  } catch(error) {
    return {ok:false,reason:String(error?.message||error)};
  }
}

module.exports={cookieConsentRejectSource,installCookieConsentRefusal};
