export type HomePromoCardInput = {
  id?: string;
  title: string;
  description: string;
  imageFile: string;
  linkUrl: string;
  imageAlt?: string;
  openInNewWindow?: boolean;
};

export type HomePromoCard = {
  id: string;
  title: string;
  description: string;
  imageFile: string;
  linkUrl: string;
  imageAlt: string;
  openInNewWindow: boolean;
};

function normalizeText(value: string, fallback: string): string {
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : fallback;
}

function normalizeCardId(value: string, fallback: string): string {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-_]+/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized.length > 0 ? normalized : fallback;
}

export function promoCard(input: HomePromoCardInput): HomePromoCard {
  return {
    id: normalizeCardId(input.id ?? "", "promo"),
    title: normalizeText(input.title, "No title"),
    description: normalizeText(input.description, "Description is not set."),
    imageFile: normalizeText(input.imageFile, ""),
    linkUrl: normalizeText(input.linkUrl, "https://example.com"),
    imageAlt: normalizeText(input.imageAlt ?? "", input.title),
    openInNewWindow: input.openInNewWindow ?? true,
  };
}

export function promoCardBuilder(
  ...cards: HomePromoCardInput[]
): HomePromoCard[] {
  const seen = new Set<string>();

  return cards.map((card, index) => {
    const normalized = promoCard(card);
    const fallbackId = `promo-${index + 1}`;
    let nextId = normalized.id === "promo" ? fallbackId : normalized.id;

    if (seen.has(nextId)) {
      let suffix = 2;
      while (seen.has(`${nextId}-${suffix}`)) {
        suffix += 1;
      }
      nextId = `${nextId}-${suffix}`;
    }

    seen.add(nextId);

    return {
      ...normalized,
      id: nextId,
    };
  });
}

export const HOME_PROMO_CARDS: HomePromoCard[] = promoCardBuilder(
  {
    id: "tbw-boosty",
    title: "Хотите поддержать сервер?",
    description:
      "Это вы можете сделать на нашем Boosty, купив подписку Bronze/Gold/Platinum",
    imageFile: "Boosty_logo.png",
    linkUrl: "https://boosty.to/tbwcorp",
  },
  /*{
    id: "promo-countercraft-cup",
    title: "Counter Craft Cup",
    description:
      "Турнир по Контр Крафт. Призовой фонд 5к тенге",
    imageFile: "Counter Craft.png",
    linkUrl: "https://t.me/tytweselooo",
  },*/
  {
    id: "tbw-telegram",
    title: "Смотрите свежие новости!",
    description:
      "В нашем TG-канале регулярно появляются новостные посты для вас - наших игроков. Подписывайтесь и следите за новостями!",
    imageFile: "Telegram_logo.png",
    linkUrl: "https://t.me/tytweselooo",
  },
  {
    id: "tbw-youtube",
    title: "Интересный контент на нашем канале!",
    description:
      "Вы бы хотели видеть регулярные и интересные видео? Скорей заходите на наш Youtube-канал, где почти каждую неделю выходят новые и увлекательные видео.",
    imageFile: "Youtube_logo.png",
    linkUrl: "https://www.youtube.com/@TBWhorizon",
  },
  {
    id: "swag-telegram",
    title: "Когда-то хотели заказать СВАГУ?",
    description:
      "Вам нужен СВЭГ скин, мод, может быть вам требуется такой же лаунчер? Всё это и намного больше Вы сможете найти в Swag Studio. Всю подробную информацию можете увидеть в нашем TG-канале.",
    imageFile: "Swag_logo.png",
    linkUrl: "https://t.me/swagstudiohq",
  },
);
