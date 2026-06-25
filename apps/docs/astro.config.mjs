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
					label: 'Guía de Inicio',
					items: [
						{ label: 'Introducción a Syntrix', slug: 'introduccion' },
						{ label: 'Instalación y Setup', slug: 'setup' },
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
