// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
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
						{ label: 'Sincronización P2P (Iroh)', slug: 'arquitectura/sincronizacion' },
						{ label: 'Estructura de Namespaces', slug: 'arquitectura/namespaces' },
					],
				},
				{
					label: 'Referencia',
					items: [
						{ label: 'Comandos de Tauri', slug: 'referencia/comandos-tauri' },
						{ label: 'Esquemas y Tipos', slug: 'referencia/esquemas' },
						{ label: 'Rustdoc: Admin Core', link: '/rustdoc/syntrix_admin_lib/index.html', attrs: { target: '_blank' } },
						{ label: 'Rustdoc: Client Core', link: '/rustdoc/syntrix_client_lib/index.html', attrs: { target: '_blank' } },
						{ label: 'Rustdoc: Iroh Docs Engine', link: '/rustdoc/iroh_syntrix_docs/index.html', attrs: { target: '_blank' } },
					],
				},
				{
					label: 'Archivo de Diseño (Drafts)',
					collapsed: true,
					items: [
						{
							label: 'Propuesta y Plan',
							items: [
								{ label: 'Identidad y Pilares', slug: 'identidad' },
								{ label: 'Pitch de Ventas', slug: 'pitch' },
								{ label: 'Argumentos P2P & Iroh', slug: 'argumentos-p2p' },
								{ label: 'Plan de Ruta', slug: 'plan' },
								{ label: 'Estado Actual', slug: 'estado-actual' },
							],
						},
						{
							label: 'Arquitectura',
							items: [
								{ label: 'Arquitectura Inicial', slug: 'arquitectura' },
								{ label: 'Arquitectura Final (Draft)', slug: 'arquitectura-final' },
								{ label: 'Diseño de Workspace', slug: 'workspace' },
								{ label: 'Plan de Interfaz (UI)', slug: 'plan-ui' },
								{ label: 'Visión a Escala', slug: 'vision' },
								{ label: 'Correcciones y Consenso', slug: 'correcciones' },
							],
						},
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
});
