import React, { useState, useEffect } from "react";
import { getVersion } from '@tauri-apps/api/app';
import Image from 'next/image';


export function About() {
    const [currentVersion, setCurrentVersion] = useState<string>('0.5.2');

    useEffect(() => {
        // Get current version on mount
        getVersion().then(setCurrentVersion).catch(console.error);
    }, []);

    return (
        <div className="p-4 space-y-4 h-[80vh] overflow-y-auto">
            {/* Compact Header */}
            <div className="text-center">
                <div className="mb-3">
                    <Image
                        src="/convoic_icon_128.png"
                        alt="Convoic Logo"
                        width={96}
                        height={96}
                        className="mx-auto"
                        priority
                    />
                </div>
                <h1 className="text-xl font-bold text-gray-900">Convoic</h1>
                <span className="text-sm text-gray-500"> v{currentVersion}</span>
                <p className="text-medium text-gray-600 mt-1">
                    Заметки и резюме встреч в реальном времени, не покидающие ваш компьютер.
                </p>
            </div>

            {/* Features Grid - Compact */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-gray-800">Чем Convoic отличается</h2>
                <div className="grid grid-cols-2 gap-2">
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Приватность прежде всего</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Ваши данные и обработка ИИ остаются на вашем устройстве. Без облака, без утечек.</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Любая модель</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Локальная open-source модель? Отлично. Внешний API? Тоже подходит. Без привязки.</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Экономия</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Избегайте оплаты поминутно — запускайте модели локально (или платите только за выбранные вызовы).</p>
                    </div>
                    <div className="bg-gray-50 rounded p-3 hover:bg-gray-100 transition-colors">
                        <h3 className="font-bold text-sm text-gray-900 mb-1">Работает везде</h3>
                        <p className="text-xs text-gray-600 leading-relaxed">Google Meet, Zoom, Teams — онлайн или офлайн.</p>
                    </div>
                </div>
            </div>
        </div>

    )
}
