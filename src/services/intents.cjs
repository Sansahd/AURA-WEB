const INTENTS = [
  {
    id: 'cook', label: 'Cuisiner', test: /\b(g[aâ]teau|recette|cuisin|manger|repas|p[aâ]tisserie|d[iî]ner|dejeuner|déjeuner)\b/i,
    sites: [
      ['Marmiton', 'https://www.marmiton.org/'],
      ['750g', 'https://www.750g.com/'],
      ['CuisineAZ', 'https://www.cuisineaz.com/']
    ]
  },
  {
    id: 'learn', label: 'Apprendre', test: /\b(apprendre|cours|formation|tutoriel|comprendre|r[eé]viser)\b/i,
    sites: [
      ['Khan Academy', 'https://fr.khanacademy.org/'],
      ['Wikipedia', 'https://fr.wikipedia.org/'],
      ['YouTube', 'https://www.youtube.com/']
    ]
  },
  {
    id: 'travel', label: 'Voyager', test: /\b(voyage|vacances|h[oô]tel|vol|week.?end|visiter|tourisme)\b/i,
    sites: [
      ['Wikivoyage', 'https://fr.wikivoyage.org/'],
      ['Rome2Rio', 'https://www.rome2rio.com/'],
      ['Booking', 'https://www.booking.com/']
    ]
  },
  {
    id: 'buy', label: 'Comparer / acheter', test: /\b(acheter|compar|prix|meilleur|pc|ordinateur|smartphone|produit)\b/i,
    sites: [
      ['Idealo', 'https://www.idealo.fr/'],
      ['Les Numériques', 'https://www.lesnumeriques.com/'],
      ['Dealabs', 'https://www.dealabs.com/']
    ]
  },
  {
    id: 'dev', label: 'Développer', test: /\b(coder|code|d[eé]velopper|github|javascript|python|application|api|bug)\b/i,
    sites: [
      ['GitHub', 'https://github.com/'],
      ['MDN', 'https://developer.mozilla.org/fr/'],
      ['Stack Overflow', 'https://stackoverflow.com/']
    ]
  }
];

function detectIntent(query = '') {
  const clean = String(query).trim();
  for (const intent of INTENTS) {
    if (intent.test.test(clean)) return { id: intent.id, label: intent.label, sites: intent.sites };
  }
  return { id: 'search', label: 'Rechercher', sites: [] };
}
module.exports = { detectIntent };
