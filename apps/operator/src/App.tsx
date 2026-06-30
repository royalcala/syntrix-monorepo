import { Routes, Route, Navigate } from "react-router-dom";

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/" element={<Dashboard />} />
      <Route path="/tenants" element={<TenantList />} />
      <Route path="/tenants/:id" element={<TenantDetail />} />
      <Route path="/rate-cards" element={<RateCards />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}

function LoginPage() {
  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50">
      <div className="bg-white p-8 rounded-xl shadow-sm border max-w-sm w-full">
        <h1 className="text-xl font-bold mb-6">Operator Login</h1>
        <form className="space-y-4">
          <input placeholder="Email" className="w-full h-10 px-3 border rounded-lg text-sm" />
          <input type="password" placeholder="Password" className="w-full h-10 px-3 border rounded-lg text-sm" />
          <button className="w-full h-10 bg-gray-900 text-white rounded-lg text-sm font-medium">Sign in</button>
        </form>
      </div>
    </div>
  );
}

function Dashboard() {
  return <div className="p-8"><h1 className="text-2xl font-bold">Dashboard</h1></div>;
}

function TenantList() {
  return <div className="p-8"><h1 className="text-2xl font-bold">Tenants</h1></div>;
}

function TenantDetail() {
  return <div className="p-8"><h1 className="text-2xl font-bold">Tenant Detail</h1></div>;
}

function RateCards() {
  return <div className="p-8"><h1 className="text-2xl font-bold">Rate Cards</h1></div>;
}
