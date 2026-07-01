// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import mermaid from 'astro-mermaid';

// https://astro.build/config
export default defineConfig({
	integrations: [
		mermaid(),
		starlight({
			title: 'Syntrix Docs',
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/royalcala/syntrix-monorepo' }],
			sidebar: [
				{
					label: 'Guías de Desarrollo',
					items: [
						{ label: 'Introducción a Syntrix', slug: 'introduccion' },
						{ label: 'Instalación y Setup', slug: 'setup' },
						{ label: 'Desarrollo Frontend (React)', slug: 'guias/frontend' },
						{ label: 'Desarrollo Backend (Rust)', slug: 'guias/backend' },
					],
				},
				{
					label: 'Arquitectura Core',
					items: [
						{ label: 'Motor de Base de Datos', slug: 'arquitectura/database' },
						{ label: 'Sincronización P2P (libp2p)', slug: 'arquitectura/sincronizacion' },
						{ label: 'Estructura de Namespaces', slug: 'arquitectura/namespaces' },
					],
				},
				{
					label: 'Producto',
					items: [
						{ label: 'Estado Actual', slug: 'estado-actual' },
						{ label: 'Hoja de Ruta', slug: 'plan' },
					],
				},
				{
					label: 'Referencia',
					items: [
						{ label: 'Comandos de Tauri', slug: 'referencia/comandos-tauri' },
						{ label: 'Esquemas y Tipos', slug: 'referencia/esquemas' },
						{ label: 'Rustdoc: Admin Core', slug: 'referencia/rustdoc-admin' },
						{ label: 'Rustdoc: Client Core', slug: 'referencia/rustdoc-client' },
						{ label: 'Rustdoc: libp2p Network', slug: 'referencia/rustdoc-iroh' },
					],
				},
				{
					label: 'Archivo de Diseño (Drafts)',
					collapsed: true,
					items: [
						{ label: 'Identidad y Pilares', slug: 'identidad' },
						{ label: 'Pitch de Ventas', slug: 'pitch' },
						{ label: 'Argumentos P2P & Iroh', slug: 'argumentos-p2p' },
						{ label: 'Visión a Escala', slug: 'vision' },
						{
							label: 'Investigación',
							items: [
								{ label: 'Ecosistema Horizontal', slug: 'horizontal-research/ecosistema-horizontal' },
							],
						},
					],
				},
			],
		}),
	],
	markdown: {
		syntaxHighlight: {
			excludeLangs: ['mermaid'],
		},
	},
});
