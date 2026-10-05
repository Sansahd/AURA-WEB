'use strict';

(() => {
  const q = (selector) => document.querySelector(selector);
  const panel = q('#quantic-suite-panel');
  const title = q('#quantic-suite-title');
  const body = q('#quantic-suite-body');
  const close = q('#quantic-suite-close');
  const sideButton = q('#sidestage');
  const mindsLink = q('#quantic-minds-link');
  const VAULT_KEY = 'quantic-vault-v1';
  const PBKDF2_ITERATIONS = 310000;
  const QUANTIC_MAIL_URL = 'https://mediumorchid-badger-314305.hostingersite.com/mail/';
  let vaultSession = null;
  let vaultLockTimer = null;

  function escapeHtml(value = '') {
    return String(value).replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]));
  }

  function bytesToBase64(bytes) {
    let binary = '';
    const array = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    for (const byte of array) binary += String.fromCharCode(byte);
    return btoa(binary);
  }

  function base64ToBytes(value) {
    const binary = atob(value);
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
    return out;
  }

  async function deriveVaultKey(passphrase, salt) {
    const material = await crypto.subtle.importKey(
      'raw',
      new TextEncoder().encode(passphrase),
      'PBKDF2',
      false,
      ['deriveKey']
    );
    return crypto.subtle.deriveKey(
      { name: 'PBKDF2', hash: 'SHA-256', salt, iterations: PBKDF2_ITERATIONS },
      material,
      { name: 'AES-GCM', length: 256 },
      false,
      ['encrypt', 'decrypt']
    );
  }

  async function encryptVault(key, entries, salt) {
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const plaintext = new TextEncoder().encode(JSON.stringify(entries));
    const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, plaintext);
    return {
      v: 1,
      kdf: 'PBKDF2-SHA256',
      iterations: PBKDF2_ITERATIONS,
      cipher: 'AES-256-GCM',
      salt: bytesToBase64(salt),
      iv: bytesToBase64(iv),
      data: bytesToBase64(ciphertext),
      updatedAt: new Date().toISOString()
    };
  }

  async function decryptVault(passphrase, blob) {
    const salt = base64ToBytes(blob.salt);
    const iv = base64ToBytes(blob.iv);
    const key = await deriveVaultKey(passphrase, salt);
    const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, base64ToBytes(blob.data));
    const entries = JSON.parse(new TextDecoder().decode(plaintext));
    return { key, salt, entries: Array.isArray(entries) ? entries : [] };
  }

  function readVaultBlob() {
    try { return JSON.parse(localStorage.getItem(VAULT_KEY) || 'null'); } catch { return null; }
  }

  function scheduleVaultLock() {
    clearTimeout(vaultLockTimer);
    vaultLockTimer = setTimeout(() => {
      vaultSession = null;
      if (panel && !panel.classList.contains('hidden') && panel.dataset.kind === 'vault') renderVault();
    }, 5 * 60 * 1000);
  }

  function openPanel(kind, heading) {
    if (!panel || !title || !body) return;
    panel.dataset.kind = kind;
    title.textContent = heading;
    body.replaceChildren();
    panel.classList.remove('hidden');
    panel.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }

  function textNode(tag, text, className = '') {
    const node = document.createElement(tag);
    if (className) node.className = className;
    node.textContent = text;
    return node;
  }

  function actionButton(label, onClick) {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = label;
    button.onclick = onClick;
    return button;
  }

  async function renderShield() {
    openPanel('shield', 'Quantic Shield');
    const current = await window.quantic.state().catch(() => ({}));
    const mode = current?.settings?.networkMode === 'private' ? 'Privé · Tor' : 'Compatible';
    const items = [
      'Filtrage réseau léger des domaines publicitaires et de pistage connus.',
      'Géolocalisation, notifications, USB, série, HID et accès réseau local refusés par défaut.',
      'WebRTC configuré pour éviter l’UDP non proxifié.',
      `Routage actuel : ${mode}.`,
      'Aucune injection DOM lourde dans le chemin critique de navigation.'
    ];
    body.append(textNode('p', 'Protection active sans proxy obligatoire ni analyse distante avant affichage.'));
    const list = textNode('div', '', 'quantic-suite-list');
    for (const item of items) list.append(textNode('div', item, 'quantic-suite-item'));
    body.append(list);
    const actions = textNode('div', '', 'quantic-suite-actions');
    actions.append(actionButton('Ouvrir les paramètres', () => window.quantic.navigate('quantic://settings')));
    body.append(actions);
  }

  function renderVaultList() {
    if (!vaultSession) return renderVault();
    openPanel('vault', 'Quantic Vault · bêta locale');
    body.append(textNode('p', 'Coffre local chiffré AES-256-GCM. La clé est dérivée de votre phrase secrète uniquement à l’ouverture du coffre. Aucun serveur Quantic n’est contacté.'));

    const list = textNode('div', '', 'quantic-suite-list');
    for (const [index, item] of vaultSession.entries.entries()) {
      const row = textNode('div', '', 'quantic-suite-item');
      row.innerHTML = `<strong>${escapeHtml(item.title || 'Entrée')}</strong><div>${escapeHtml(item.login || '')}</div><div class="quantic-suite-note">Secret masqué</div>`;
      const actions = textNode('div', '', 'quantic-suite-actions');
      actions.append(
        actionButton('Afficher', () => {
          const note = row.querySelector('.quantic-suite-note');
          note.textContent = note.textContent === 'Secret masqué' ? String(item.secret || '') : 'Secret masqué';
          scheduleVaultLock();
        }),
        actionButton('Supprimer', async () => {
          vaultSession.entries.splice(index, 1);
          await persistVault();
          renderVaultList();
        })
      );
      row.append(actions);
      list.append(row);
    }
    if (!vaultSession.entries.length) list.append(textNode('div', 'Coffre vide.', 'quantic-suite-item'));
    body.append(list);

    const titleInput = document.createElement('input');
    titleInput.placeholder = 'Nom / service';
    const loginInput = document.createElement('input');
    loginInput.placeholder = 'Identifiant (facultatif)';
    const secretInput = document.createElement('textarea');
    secretInput.placeholder = 'Secret, mot de passe ou note privée';
    body.append(titleInput, loginInput, secretInput);

    const actions = textNode('div', '', 'quantic-suite-actions');
    actions.append(
      actionButton('Ajouter au coffre', async () => {
        if (!secretInput.value.trim()) return;
        vaultSession.entries.push({
          id: crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random()}`,
          title: titleInput.value.trim().slice(0, 160),
          login: loginInput.value.trim().slice(0, 240),
          secret: secretInput.value,
          createdAt: new Date().toISOString()
        });
        await persistVault();
        renderVaultList();
      }),
      actionButton('Verrouiller', () => {
        vaultSession = null;
        clearTimeout(vaultLockTimer);
        renderVault();
      })
    );
    body.append(actions, textNode('div', 'Bêta : pas d’autoremplissage. Le coffre reste volontairement séparé du moteur de navigation.', 'quantic-suite-note'));
  }

  async function persistVault() {
    if (!vaultSession) return;
    const blob = await encryptVault(vaultSession.key, vaultSession.entries, vaultSession.salt);
    localStorage.setItem(VAULT_KEY, JSON.stringify(blob));
    scheduleVaultLock();
  }

  function renderVault() {
    openPanel('vault', 'Quantic Vault · bêta locale');
    if (vaultSession) return renderVaultList();
    const existing = readVaultBlob();
    body.append(textNode('p', existing
      ? 'Entrez votre phrase secrète pour déchiffrer le coffre local.'
      : 'Créez une phrase secrète. Elle ne sera ni envoyée ni stockée en clair. Si vous la perdez, Quantic ne peut pas récupérer le coffre.'));
    const pass = document.createElement('input');
    pass.type = 'password';
    pass.autocomplete = 'off';
    pass.placeholder = existing ? 'Phrase secrète' : 'Nouvelle phrase secrète (12 caractères minimum)';
    body.append(pass);
    const status = textNode('div', '', 'quantic-suite-note');
    const actions = textNode('div', '', 'quantic-suite-actions');
    actions.append(actionButton(existing ? 'Déverrouiller' : 'Créer le coffre', async () => {
      const phrase = pass.value;
      if (phrase.length < 12) {
        status.textContent = 'Utilisez au moins 12 caractères.';
        return;
      }
      try {
        if (existing) {
          vaultSession = await decryptVault(phrase, existing);
        } else {
          const salt = crypto.getRandomValues(new Uint8Array(16));
          const key = await deriveVaultKey(phrase, salt);
          vaultSession = { key, salt, entries: [] };
          await persistVault();
        }
        pass.value = '';
        scheduleVaultLock();
        renderVaultList();
      } catch {
        status.textContent = 'Impossible de déchiffrer le coffre : phrase secrète incorrecte ou données endommagées.';
      }
    }));
    if (existing) {
      actions.append(actionButton('Effacer le coffre', () => {
        if (!confirm('Effacer définitivement le coffre local Quantic ?')) return;
        localStorage.removeItem(VAULT_KEY);
        vaultSession = null;
        renderVault();
      }));
    }
    body.append(actions, status);
  }

  function renderProof() {
    openPanel('proof', 'Quantic Proof');
    body.append(textNode('p', 'Vérification locale d’intégrité. Sélectionnez un fichier : son SHA-256 est calculé dans le navigateur, sans envoi réseau.'));
    const input = document.createElement('input');
    input.type = 'file';
    const result = textNode('div', 'Aucun fichier sélectionné.', 'quantic-proof-hash');
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      result.textContent = `Calcul SHA-256 de ${file.name}…`;
      const digest = await crypto.subtle.digest('SHA-256', await file.arrayBuffer());
      const hex = [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, '0')).join('');
      result.replaceChildren(textNode('div', file.name), textNode('code', hex));
    };
    body.append(input, result, textNode('div', 'Les releases Quantic fournissent aussi SHA256SUMS.txt et un manifeste Proof de build.', 'quantic-suite-note'));
  }

  async function renderTransparency() {
    openPanel('transparency', 'Quantic Transparency');
    const current = await window.quantic.state().catch(() => ({}));
    const settings = current?.settings || {};
    const side = current?.sideStage || {};
    const rows = [
      ['Télémétrie Quantic', 'Aucun SDK de télémétrie first-party dans le contrat de release'],
      ['Profil', settings.networkMode === 'private' ? 'Privé éphémère' : 'Local persistant'],
      ['SideStage', side.enabled === false ? 'Désactivé' : 'Activé'],
      ['Cache à la fermeture', settings.clearCacheOnExit === false ? 'Conservé pour la performance' : 'Effacé'],
      ['DRM', 'CastLabs ECS / Widevine sur les builds VMP signées'],
      ['Preuve de build', 'SHA-256 + manifestes produits à la génération de release']
    ];
    const list = textNode('div', '', 'quantic-suite-list');
    for (const [label, value] of rows) {
      const row = textNode('div', '', 'quantic-suite-item');
      row.append(textNode('strong', label), textNode('div', value));
      list.append(row);
    }
    body.append(list);
  }

  function renderMail() {
    openPanel('mail', 'Quantic Mail');
    body.append(
      textNode('p', 'Quantic Mail est la messagerie native de l’écosystème Quantic : identité Quantic, chiffrement de bout en bout et stockage local-first.'),
      textNode('div', 'Le navigateur ouvre directement le réseau Quantic Mail. Proton n’est plus utilisé comme service mail par défaut de Glide.', 'quantic-suite-item')
    );
    const list = textNode('div', '', 'quantic-suite-list');
    for (const item of [
      'Identités Quantic indépendantes des comptes Gmail ou Proton.',
      'Messages chiffrés côté client avant transit.',
      'Coffre et identité restaurables sans transformer le serveur en propriétaire de vos données.'
    ]) list.append(textNode('div', item, 'quantic-suite-item'));
    body.append(list);
    const actions = textNode('div', '', 'quantic-suite-actions');
    actions.append(actionButton('Ouvrir Quantic Mail', () => window.quantic.navigate(QUANTIC_MAIL_URL)));
    body.append(actions);
  }

  function renderAnalysis() {
    openPanel('analysis', 'Providence · Analyse à la demande');
    body.append(
      textNode('p', 'Les fonctions d’analyse restent séparées du chemin critique de navigation. Elles ne sont chargées que lorsque vous les demandez.'),
      textNode('div', 'Cette séparation protège le démarrage, la navigation et la consommation mémoire de Glide.', 'quantic-suite-item')
    );
    const actions = textNode('div', '', 'quantic-suite-actions');
    actions.append(actionButton('Ouvrir l’assistant d’analyse', () => {
      panel.classList.add('hidden');
      window.quantic.toggleAi();
    }));
    body.append(actions);
  }

  const handlers = {
    shield: renderShield,
    vault: renderVault,
    proof: renderProof,
    transparency: renderTransparency,
    mail: renderMail,
    analysis: renderAnalysis
  };

  for (const button of document.querySelectorAll('[data-quantic-suite]')) {
    button.addEventListener('click', () => {
      const fn = handlers[button.dataset.quanticSuite];
      if (fn) Promise.resolve(fn()).catch(() => {});
    });
  }

  close?.addEventListener('click', () => panel.classList.add('hidden'));

  sideButton?.addEventListener('click', () => {
    window.quantic.setSetting('sideStageAction', 'toggle').catch(() => {});
  });

  mindsLink?.addEventListener('click', (event) => {
    event.preventDefault();
    window.quantic.navigate(mindsLink.href).catch(() => {});
  });
})();
