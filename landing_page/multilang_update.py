#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Generate translations for MonoSub landing page
"""

# Core translations for all 24 languages
# Focus on key marketing messages

translations = {
    "en": {
        "hero_title": "Stop Paying Monthly. Own Your Subtitle Tool Forever.",
        "hero_cta": "Get Lifetime Access - $39",
        "feature_1": "100% Local Processing",
        "feature_2": "Buy Once, Use Forever",
        "pricing_save": "Save over $1,000 long-term"
    },
    "es": {
        "hero_title": "Deja de Pagar Mensualmente. Posee Tu Herramienta de Subtítulos Para Siempre.",
        "hero_cta": "Obtener Acceso de Por Vida - €35",
        "feature_1": "Procesamiento 100% Local",
        "feature_2": "Compra Una Vez, Usa Para Siempre",
        "pricing_save": "Ahorra más de €1,000 a largo plazo"
    },
    "es-419": {
        "hero_title": "Deja de Pagar Mensualmente. Tu Herramienta de Subtítulos Para Siempre.",
        "hero_cta": "Obtén Acceso de Por Vida - $39",
        "feature_1": "Procesamiento 100% Local",
        "feature_2": "Compra Una Vez, Usa Para Siempre",
        "pricing_save": "Ahorra más de $1,000 a largo plazo"
    },
    "ru": {
        "hero_title": "Хватит Платить Каждый Месяц. Владей Своим Инструментом Навсегда.",
        "hero_cta": "Получить Пожизненный Доступ - ₽3,500",
        "feature_1": "100% Локальная Обработка",
        "feature_2": "Купи Один Раз, Используй Вечно",
        "pricing_save": "Экономия более ₽100,000"
    },
    "pt-BR": {
        "hero_title": "Pare de Pagar Mensalmente. Possua Sua Ferramenta Para Sempre.",
        "hero_cta": "Obter Acesso Vitalício - R$ 199",
        "feature_1": "Processamento 100% Local",
        "feature_2": "Compre Uma Vez, Use Para Sempre",
        "pricing_save": "Economize mais de R$ 5,000"
    },
    "zh-CN": {
        "hero_title": "停止按月付费，永久拥有字幕工具",
        "hero_cta": "获取终身访问权 - ¥299",
        "feature_1": "100% 本地处理",
        "feature_2": "一次购买终身使用",
        "pricing_save": "长期节省超过 ¥8,000"
    },
    "de": {
        "hero_title": "Hören Sie auf, monatlich zu zahlen. Besitzen Sie Ihr Untertitel-Tool für immer.",
        "hero_cta": "Lebenslangen Zugang Erhalten - €35",
        "feature_1": "100% Lokale Verarbeitung",
        "feature_2": "Einmal Kaufen, Für Immer Nutzen",
        "pricing_save": "Sparen Sie langfristig über €1,000"
    },
    "fr": {
        "hero_title": "Arrêtez de Payer Mensuellement. Possédez Votre Outil de Sous-titres Pour Toujours.",
        "hero_cta": "Obtenir un Accès à Vie - €35",
        "feature_1": "Traitement 100% Local",
        "feature_2": "Achetez Une Fois, Utilisez Pour Toujours",
        "pricing_save": "Économisez plus de €1,000"
    },
    "ja": {
        "hero_title": "月額払いをやめて、字幕ツールを永久に所有しましょう",
        "hero_cta": "生涯アクセスを取得 - ¥5,900",
        "feature_1": "100%ローカル処理",
        "feature_2": "一度購入、永久使用",
        "pricing_save": "長期的に¥300,000以上節約"
    },
    "ko": {
        "hero_title": "월 구독을 중단하고 자막 도구를 영원히 소유하세요",
        "hero_cta": "평생 액세스 받기 - ₩59,000",
        "feature_1": "100% 로컬 처리",
        "feature_2": "한 번 구매, 영원히 사용",
        "pricing_save": "장기적으로 ₩3,000,000 이상 절약"
    }
}

import json
with open('translations_core.json', 'w', encoding='utf-8') as f:
    json.dump(translations, f, ensure_ascii=False, indent=2)

print("✓ Core translations generated")
print(f"✓ Languages covered: {len(translations)}")
