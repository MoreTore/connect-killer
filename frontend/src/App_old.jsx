import React from 'react';
import { Routes, Route } from 'react-router-dom';
import Login from './Login.jsx';
import Signup from './Signup.jsx';  // Assume you have a Signup component

function App() {
  return (
    <div className="App">
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/register" element={<Signup />} />
      </Routes>
    </div>
  );
}

export default App;